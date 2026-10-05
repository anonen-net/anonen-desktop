use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

const GPU_ATTEMPT_MARKER: &str = "gpu-attempt.marker";

const HEADROOM_NUMERATOR: u64 = 3;
const HEADROOM_DENOMINATOR: u64 = 2;

const BYTES_PER_MB: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuFit {
    Fits,

    TooSmall { needed_mb: u64, available_mb: u64 },

    Unknown,
}

pub fn whisper_gpu_fit(model_bytes: u64, free_vram: u64, total_vram: u64) -> GpuFit {
    if model_bytes == 0 {
        return GpuFit::Unknown;
    }
    let available = if free_vram > 0 { free_vram } else { total_vram };
    if available == 0 {
        return GpuFit::Unknown;
    }
    let needed = model_bytes.saturating_mul(HEADROOM_NUMERATOR) / HEADROOM_DENOMINATOR;
    if needed > available {
        GpuFit::TooSmall {
            needed_mb: needed / BYTES_PER_MB,
            available_mb: available / BYTES_PER_MB,
        }
    } else {
        GpuFit::Fits
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GpuFallbackNotice {
    pub reason: &'static str,
    pub needed_mb: u64,
    pub available_mb: u64,
}

pub const GPU_FALLBACK_EVENT: &str = "whisper-gpu-fallback";

pub const REASON_VRAM: &str = "vram";

pub const REASON_PREVIOUS_CRASH: &str = "previous_crash";

pub fn gpu_host_block_reason(has_fma3: bool, emulated_x64_on_arm64: bool) -> Option<&'static str> {
    if !has_fma3 {
        return Some("CPU lacks FMA3 support (ggml's Vulkan backend would crash)");
    }
    if emulated_x64_on_arm64 {
        return Some("Windows x64 build is running under emulation on an ARM64 host");
    }
    None
}

pub struct OneShot(AtomicBool);

impl OneShot {
    pub const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    pub fn take(&self) -> bool {
        !self.0.swap(true, Ordering::Relaxed)
    }
}

impl Default for OneShot {
    fn default() -> Self {
        Self::new()
    }
}

pub static VRAM_NOTICE: OneShot = OneShot::new();

pub static PREVIOUS_CRASH_NOTICE: OneShot = OneShot::new();

static GPU_DISABLED_THIS_RUN: AtomicBool = AtomicBool::new(false);

pub fn disable_gpu_for_this_run() {
    GPU_DISABLED_THIS_RUN.store(true, Ordering::Relaxed);
}

pub fn gpu_disabled_this_run() -> bool {
    GPU_DISABLED_THIS_RUN.load(Ordering::Relaxed)
}

pub fn take_previous_crash_marker(log_dir: &Path) -> bool {
    let path = log_dir.join(GPU_ATTEMPT_MARKER);
    match fs::remove_file(&path) {
        Ok(()) => true,

        Err(_) => false,
    }
}

pub struct GpuAttemptMarker {
    path: PathBuf,
}

impl GpuAttemptMarker {
    pub fn begin(log_dir: &Path) -> Option<Self> {
        let path = log_dir.join(GPU_ATTEMPT_MARKER);

        fs::write(&path, b"gpu inference in progress").ok()?;
        Some(Self { path })
    }
}

impl Drop for GpuAttemptMarker {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = BYTES_PER_MB;

    #[test]
    fn a_large_model_on_a_small_card_falls_back_to_the_cpu() {
        let fit = whisper_gpu_fit(1100 * MB, 1000 * MB, 2000 * MB);
        assert_eq!(
            fit,
            GpuFit::TooSmall {
                needed_mb: 1650,
                available_mb: 1000
            }
        );
    }

    #[test]
    fn the_same_model_fits_when_the_card_is_empty() {
        assert_eq!(
            whisper_gpu_fit(1100 * MB, 7000 * MB, 8000 * MB),
            GpuFit::Fits
        );
    }

    #[test]
    fn the_headroom_is_what_makes_the_difference() {
        assert_eq!(
            whisper_gpu_fit(1000 * MB, 1000 * MB, 4000 * MB),
            GpuFit::TooSmall {
                needed_mb: 1500,
                available_mb: 1000
            }
        );
        assert_eq!(
            whisper_gpu_fit(1000 * MB, 1500 * MB, 4000 * MB),
            GpuFit::Fits
        );
    }

    #[test]
    fn a_driver_that_reports_no_free_memory_is_judged_on_the_total() {
        assert_eq!(whisper_gpu_fit(1100 * MB, 0, 8000 * MB), GpuFit::Fits);
        assert_eq!(
            whisper_gpu_fit(1100 * MB, 0, 1000 * MB),
            GpuFit::TooSmall {
                needed_mb: 1650,
                available_mb: 1000
            }
        );
    }

    #[test]
    fn nothing_to_compare_leaves_the_gpu_path_alone() {
        assert_eq!(whisper_gpu_fit(0, 8000 * MB, 8000 * MB), GpuFit::Unknown);
        assert_eq!(whisper_gpu_fit(1100 * MB, 0, 0), GpuFit::Unknown);
    }

    #[test]
    fn a_marker_left_behind_means_the_previous_run_died_on_the_gpu() {
        let dir = tempfile::tempdir().unwrap();
        {
            let _marker = GpuAttemptMarker::begin(dir.path()).unwrap();

            std::mem::forget(_marker);
        }
        assert!(take_previous_crash_marker(dir.path()));
    }

    #[test]
    fn a_run_that_returned_normally_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        drop(GpuAttemptMarker::begin(dir.path()).unwrap());
        assert!(!take_previous_crash_marker(dir.path()));
    }

    #[test]
    fn a_panic_is_not_mistaken_for_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let _ = std::panic::catch_unwind(move || {
            let _marker = GpuAttemptMarker::begin(&path).unwrap();
            panic!("simulated transcription panic");
        });
        assert!(!take_previous_crash_marker(dir.path()));
    }

    #[test]
    fn reading_the_marker_clears_it_so_the_gpu_is_not_shut_off_for_good() {
        let dir = tempfile::tempdir().unwrap();
        std::mem::forget(GpuAttemptMarker::begin(dir.path()).unwrap());

        assert!(take_previous_crash_marker(dir.path()));
        assert!(
            !take_previous_crash_marker(dir.path()),
            "2 回目は残っていない"
        );
    }

    #[test]
    fn a_notice_is_shown_once_and_then_stays_quiet() {
        let notice = OneShot::new();
        assert!(notice.take());
        assert!(!notice.take());
        assert!(!notice.take());
    }

    #[test]
    fn a_cpu_without_fma3_is_blocked_like_an_emulated_x64() {
        assert!(gpu_host_block_reason(true, false).is_none());
        assert!(gpu_host_block_reason(false, false).is_some());
        assert!(gpu_host_block_reason(true, true).is_some());
        assert!(gpu_host_block_reason(false, true).is_some());
    }
}
