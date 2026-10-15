use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

pub const MAX_AGE: Duration = Duration::from_secs(60);

pub const WAIT_BEFORE_SEND: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub mono: Instant,
    pub wall: SystemTime,
}

impl Stamp {
    pub fn now() -> Self {
        Self {
            mono: Instant::now(),
            wall: SystemTime::now(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CatalogClock {
    fetched: Option<Stamp>,
    outdated: Option<Instant>,
}

impl CatalogClock {
    pub fn accept(&mut self, started: Stamp) -> bool {
        if self
            .fetched
            .is_some_and(|current| started.mono < current.mono)
        {
            return false;
        }
        self.fetched = Some(started);
        true
    }

    pub fn mark_outdated(&mut self, at: Instant) {
        self.outdated = Some(at);
    }

    pub fn is_stale(&self, now: Stamp) -> bool {
        let Some(fetched) = self.fetched else {
            return true;
        };
        if self.outdated.is_some_and(|at| at >= fetched.mono) {
            return true;
        }
        if now.mono.saturating_duration_since(fetched.mono) > MAX_AGE {
            return true;
        }
        match now.wall.duration_since(fetched.wall) {
            Ok(elapsed) => elapsed > MAX_AGE,
            Err(_) => true,
        }
    }
}

#[derive(Default)]
pub struct RefreshGate {
    running: Mutex<Option<Arc<Finished>>>,
}

#[derive(Default)]
struct Finished {
    done: Mutex<bool>,
    signal: Condvar,
}

struct MarkFinished(Arc<Finished>);

impl Drop for MarkFinished {
    fn drop(&mut self) {
        let mut done = self.0.done.lock().unwrap_or_else(PoisonError::into_inner);
        *done = true;
        self.0.signal.notify_all();
    }
}

impl RefreshGate {
    pub fn start_if_idle<F>(&self, work: F) -> bool
    where
        F: FnOnce() + Send + 'static,
    {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(current) = running.as_ref() {
            if !*current.done.lock().unwrap_or_else(PoisonError::into_inner) {
                return false;
            }
        }
        let finished = Arc::new(Finished::default());
        *running = Some(Arc::clone(&finished));
        drop(running);
        let spawned = std::thread::Builder::new()
            .name("catalog-refresh".into())
            .spawn({
                let finished = Arc::clone(&finished);
                move || {
                    let _mark = MarkFinished(finished);
                    work();
                }
            });
        if spawned.is_err() {
            drop(MarkFinished(finished));
            return false;
        }
        true
    }

    pub fn wait(&self, limit: Duration) -> bool {
        let current = self
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let Some(current) = current else {
            return true;
        };
        let done = current.done.lock().unwrap_or_else(PoisonError::into_inner);
        let (done, _) = current
            .signal
            .wait_timeout_while(done, limit, |finished| !*finished)
            .unwrap_or_else(PoisonError::into_inner);
        *done
    }
}

pub fn refresh_before_send(
    gate: &RefreshGate,
    is_stale: impl Fn() -> bool,
    start_refresh: impl Fn(),
    limit: Duration,
) -> bool {
    let deadline = Instant::now() + limit;
    for _ in 0..2 {
        if !is_stale() {
            return true;
        }
        start_refresh();
        if !gate.wait(deadline.saturating_duration_since(Instant::now())) {
            return false;
        }
    }
    !is_stale()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::mpsc;

    fn at(base: Stamp, mono_s: u64, wall_s: u64) -> Stamp {
        Stamp {
            mono: base.mono + Duration::from_secs(mono_s),
            wall: base.wall + Duration::from_secs(wall_s),
        }
    }

    #[test]
    fn never_fetched_is_stale() {
        assert!(CatalogClock::default().is_stale(Stamp::now()));
    }

    #[test]
    fn fresh_until_max_age_then_stale() {
        let t0 = Stamp::now();
        let mut clock = CatalogClock::default();
        assert!(clock.accept(t0));
        assert!(!clock.is_stale(t0));
        assert!(!clock.is_stale(at(t0, 60, 60)));
        assert!(clock.is_stale(at(t0, 61, 61)));
    }

    #[test]
    fn a_sleep_the_monotonic_clock_did_not_count_still_makes_it_stale() {
        let t0 = Stamp::now();
        let mut clock = CatalogClock::default();
        assert!(clock.accept(t0));
        assert!(clock.is_stale(at(t0, 10, 2 * 60 * 60)));
    }

    #[test]
    fn a_wall_clock_set_back_makes_it_stale() {
        let t0 = at(Stamp::now(), 0, 3600);
        let mut clock = CatalogClock::default();
        assert!(clock.accept(t0));
        let back = Stamp {
            mono: t0.mono + Duration::from_secs(5),
            wall: t0.wall - Duration::from_secs(600),
        };
        assert!(clock.is_stale(back));
    }

    #[test]
    fn a_failed_fetch_makes_it_stale_until_the_next_fetch() {
        let t0 = Stamp::now();
        let mut clock = CatalogClock::default();
        assert!(clock.accept(t0));
        clock.mark_outdated(at(t0, 5, 5).mono);
        assert!(clock.is_stale(at(t0, 6, 6)));
        assert!(clock.accept(at(t0, 7, 7)));
        assert!(!clock.is_stale(at(t0, 8, 8)));
    }

    #[test]
    fn a_fetch_that_started_before_a_version_change_does_not_count_as_fresh() {
        let t0 = Stamp::now();
        let mut clock = CatalogClock::default();
        assert!(clock.accept(t0));
        let a_started = at(t0, 70, 70);
        clock.mark_outdated(at(t0, 71, 71).mono);
        assert!(clock.accept(a_started));
        assert!(clock.is_stale(at(t0, 72, 72)));
        assert!(clock.accept(at(t0, 73, 73)));
        assert!(!clock.is_stale(at(t0, 74, 74)));
    }

    #[test]
    fn a_late_older_fetch_does_not_overwrite_a_newer_one() {
        let t0 = Stamp::now();
        let mut clock = CatalogClock::default();
        assert!(clock.accept(at(t0, 2, 2)));
        assert!(!clock.accept(t0));
        assert!(!clock.is_stale(at(t0, 3, 3)));
    }

    fn counting(starts: &AtomicUsize) -> usize {
        starts.load(Ordering::SeqCst)
    }

    #[test]
    fn a_fresh_catalog_is_used_without_fetching() {
        let gate = RefreshGate::default();
        let starts = AtomicUsize::new(0);
        let fresh = refresh_before_send(
            &gate,
            || false,
            || {
                starts.fetch_add(1, Ordering::SeqCst);
            },
            Duration::from_secs(5),
        );
        assert!(fresh);
        assert_eq!(counting(&starts), 0);
    }

    #[test]
    fn a_stale_catalog_is_fetched_once_before_sending() {
        let gate = RefreshGate::default();
        let stale = Arc::new(AtomicBool::new(true));
        let starts = AtomicUsize::new(0);
        let fresh = refresh_before_send(
            &gate,
            || stale.load(Ordering::SeqCst),
            || {
                starts.fetch_add(1, Ordering::SeqCst);
                let stale = Arc::clone(&stale);
                gate.start_if_idle(move || stale.store(false, Ordering::SeqCst));
            },
            Duration::from_secs(5),
        );
        assert!(fresh);
        assert_eq!(counting(&starts), 1);
    }

    #[test]
    fn a_refresh_that_began_before_the_catalog_was_known_outdated_is_followed_by_one_more() {
        let gate = RefreshGate::default();
        let stale = Arc::new(AtomicBool::new(true));
        let (release, blocked) = mpsc::channel::<()>();
        assert!(gate.start_if_idle(move || {
            let _ = blocked.recv();
        }));
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            let _ = release.send(());
        });
        let starts = AtomicUsize::new(0);
        let fresh = refresh_before_send(
            &gate,
            || stale.load(Ordering::SeqCst),
            || {
                starts.fetch_add(1, Ordering::SeqCst);
                let stale = Arc::clone(&stale);
                gate.start_if_idle(move || stale.store(false, Ordering::SeqCst));
            },
            Duration::from_secs(5),
        );
        assert!(fresh);
        assert_eq!(counting(&starts), 2);
    }

    #[test]
    fn a_catalog_that_stays_stale_is_fetched_at_most_twice() {
        let gate = RefreshGate::default();
        let starts = AtomicUsize::new(0);
        let fresh = refresh_before_send(
            &gate,
            || true,
            || {
                starts.fetch_add(1, Ordering::SeqCst);
                gate.start_if_idle(|| {});
            },
            Duration::from_secs(5),
        );
        assert!(!fresh);
        assert_eq!(counting(&starts), 2);
    }

    #[test]
    fn a_hung_refresh_lets_the_sender_go_at_the_limit() {
        let gate = RefreshGate::default();
        let started = Instant::now();
        let fresh = refresh_before_send(
            &gate,
            || true,
            || {
                gate.start_if_idle(|| std::thread::sleep(Duration::from_millis(500)));
            },
            Duration::from_millis(80),
        );
        assert!(!fresh);
        assert!(started.elapsed() < Duration::from_millis(400));
    }

    #[test]
    fn the_numbers_match_android() {
        assert_eq!(MAX_AGE, Duration::from_secs(60));
        assert_eq!(WAIT_BEFORE_SEND, Duration::from_secs(3));
    }

    #[test]
    fn waiting_with_nothing_running_returns_at_once() {
        let gate = RefreshGate::default();
        let started = Instant::now();
        assert!(gate.wait(Duration::from_secs(5)));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn only_one_refresh_runs_at_a_time() {
        let gate = RefreshGate::default();
        let (release, blocked) = mpsc::channel::<()>();
        assert!(gate.start_if_idle(move || {
            let _ = blocked.recv();
        }));
        assert!(!gate.start_if_idle(|| {}));
        release.send(()).unwrap();
        assert!(gate.wait(Duration::from_secs(5)));
        assert!(gate.start_if_idle(|| {}));
        assert!(gate.wait(Duration::from_secs(5)));
    }

    #[test]
    fn the_sender_stops_waiting_at_the_limit_and_the_refresh_goes_on() {
        let gate = RefreshGate::default();
        let (release, blocked) = mpsc::channel::<()>();
        let (finished_tx, finished_rx) = mpsc::channel::<()>();
        assert!(gate.start_if_idle(move || {
            let _ = blocked.recv();
            let _ = finished_tx.send(());
        }));
        let started = Instant::now();
        assert!(!gate.wait(Duration::from_millis(50)));
        assert!(started.elapsed() >= Duration::from_millis(50));
        assert!(started.elapsed() < Duration::from_secs(2));

        release.send(()).unwrap();
        finished_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(gate.wait(Duration::from_secs(5)));
    }

    #[test]
    fn the_sender_continues_as_soon_as_the_refresh_ends() {
        let gate = RefreshGate::default();
        let (release, blocked) = mpsc::channel::<()>();
        assert!(gate.start_if_idle(move || {
            let _ = blocked.recv();
        }));
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            let _ = release.send(());
        });
        let started = Instant::now();
        assert!(gate.wait(Duration::from_secs(5)));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn a_panicking_refresh_still_counts_as_finished() {
        let gate = RefreshGate::default();
        assert!(gate.start_if_idle(|| panic!("the refresh failed hard (expected in this test)")));
        assert!(gate.wait(Duration::from_secs(5)));
        assert!(gate.start_if_idle(|| {}));
        assert!(gate.wait(Duration::from_secs(5)));
    }
}
