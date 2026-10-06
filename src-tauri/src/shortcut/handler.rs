use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use log::warn;
use tauri::{AppHandle, Emitter, Manager};

use crate::actions::ACTION_MAP;
use crate::managers::audio::AudioRecordingManager;
use crate::settings::get_settings;
use crate::transcription_coordinator::is_transcribe_binding;
use crate::TranscriptionCoordinator;

pub const SHORTCUT_PROBE_EVENT: &str = "anonen-shortcut-probe";

const PROBE_MAX_AGE: Duration = Duration::from_secs(10 * 60);

static PROBE_SINCE: Mutex<Option<Instant>> = Mutex::new(None);

pub fn set_shortcut_probe(on: bool) {
    *PROBE_SINCE.lock().unwrap_or_else(|e| e.into_inner()) =
        if on { Some(Instant::now()) } else { None };
}

pub(crate) fn probe_takes(
    since: Option<Instant>,
    now: Instant,
    main_focused: bool,
    recording: bool,
) -> bool {
    match since {
        Some(t) => main_focused && !recording && now.saturating_duration_since(t) < PROBE_MAX_AGE,
        None => false,
    }
}

fn probe_takes_now(app: &AppHandle) -> bool {
    let since = *PROBE_SINCE.lock().unwrap_or_else(|e| e.into_inner());
    if since.is_none() {
        return false;
    }
    let main_focused = app
        .get_webview_window("main")
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false);
    let recording = app
        .try_state::<Arc<AudioRecordingManager>>()
        .map_or(false, |rm| rm.is_recording());
    probe_takes(since, Instant::now(), main_focused, recording)
}

pub fn handle_shortcut_event(
    app: &AppHandle,
    binding_id: &str,
    hotkey_string: &str,
    is_pressed: bool,
) {
    log::info!(
        "[input] {} binding='{}' key='{}'",
        if is_pressed { "DOWN" } else { "UP" },
        binding_id,
        hotkey_string
    );

    let settings = get_settings(app);

    if is_transcribe_binding(binding_id) {
        if probe_takes_now(app) {
            if is_pressed {
                let _ = app.emit(SHORTCUT_PROBE_EVENT, ());
            }
            return;
        }
        if let Some(coordinator) = app.try_state::<TranscriptionCoordinator>() {
            coordinator.send_input(binding_id, hotkey_string, is_pressed, settings.push_to_talk);
        } else {
            warn!("TranscriptionCoordinator is not initialized");
        }
        return;
    }

    let Some(action) = ACTION_MAP.get(binding_id) else {
        warn!(
            "No action defined in ACTION_MAP for shortcut ID '{}'. Shortcut: '{}', Pressed: {}",
            binding_id, hotkey_string, is_pressed
        );
        return;
    };

    if binding_id == "cancel" {
        if is_pressed {
            action.start(app, binding_id, hotkey_string);
        }
        return;
    }

    if is_pressed {
        action.start(app, binding_id, hotkey_string);
    } else {
        action.stop(app, binding_id, hotkey_string);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_is_off_until_the_screen_opens() {
        assert!(!probe_takes(None, Instant::now(), true, false));
    }

    #[test]
    fn probe_takes_a_press_while_the_screen_is_in_front() {
        let t = Instant::now();
        assert!(probe_takes(
            Some(t),
            t + Duration::from_secs(5),
            true,
            false
        ));
    }

    #[test]
    fn a_press_in_another_app_still_records() {
        let t = Instant::now();
        assert!(!probe_takes(
            Some(t),
            t + Duration::from_secs(5),
            false,
            false
        ));
    }

    #[test]
    fn a_press_that_stops_a_recording_is_never_taken() {
        let t = Instant::now();
        assert!(!probe_takes(
            Some(t),
            t + Duration::from_secs(5),
            true,
            true
        ));
    }

    #[test]
    fn a_probe_left_on_expires() {
        let t = Instant::now();
        assert!(probe_takes(
            Some(t),
            t + PROBE_MAX_AGE - Duration::from_secs(1),
            true,
            false
        ));
        assert!(!probe_takes(Some(t), t + PROBE_MAX_AGE, true, false));
    }
}
