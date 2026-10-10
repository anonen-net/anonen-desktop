use crate::actions::PipelineGeneration;
use crate::managers::audio::AudioRecordingManager;
use crate::managers::transcription::TranscriptionManager;
use crate::shortcut;
use crate::TranscriptionCoordinator;
use log::info;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub use crate::clipboard::*;
pub use crate::overlay::*;
pub use crate::tray::*;

pub fn loggable_path(path: &Path) -> String {
    loggable_path_within(path, home_dir_for_logs().as_deref())
}

fn home_dir_for_logs() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .filter(|home| !home.as_os_str().is_empty())
}

fn loggable_path_within(path: &Path, home: Option<&Path>) -> String {
    match home {
        Some(home) => match path.strip_prefix(home) {
            Ok(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
            Err(_) => path.display().to_string(),
        },

        None => path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string()),
    }
}

pub fn redact_home_paths(line: &str) -> String {
    redact_home_paths_within(line, home_dir_for_logs().as_deref())
}

fn redact_home_paths_within(line: &str, home: Option<&Path>) -> String {
    let Some(home) = home.and_then(Path::to_str).filter(|h| !h.is_empty()) else {
        return line.to_string();
    };

    let haystack = line.to_ascii_lowercase();
    let needle = home.to_ascii_lowercase();

    let mut out = String::with_capacity(line.len());
    let mut rest = 0usize;
    while let Some(offset) = haystack[rest..].find(&needle) {
        let at = rest + offset;
        out.push_str(&line[rest..at]);
        out.push('~');
        rest = at + needle.len();
    }
    out.push_str(&line[rest..]);
    out
}

pub fn loggable_path_opt(path: Option<&Path>) -> String {
    match path {
        Some(path) => loggable_path(path),
        None => "<unknown>".to_string(),
    }
}

pub const QUOTED_BODY_MAX_CHARS: usize = 200;

pub const NOTIFICATION_REASON_MAX_CHARS: usize = 80;

pub fn foreign_text(raw: &str, max_chars: usize) -> String {
    let flattened: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let trimmed = flattened.trim();
    if trimmed.chars().count() > max_chars {
        format!("{}…", trimmed.chars().take(max_chars).collect::<String>())
    } else {
        trimmed.to_string()
    }
}

pub fn quoted_body(raw: &str) -> String {
    foreign_text(raw, QUOTED_BODY_MAX_CHARS)
}

pub fn notification_reason(raw: &str) -> String {
    foreign_text(raw, NOTIFICATION_REASON_MAX_CHARS)
}

#[cfg(any(test, all(target_os = "windows", target_arch = "x86_64")))]
const IMAGE_FILE_MACHINE_ARM64: u16 = 0xaa64;

#[cfg(any(test, all(target_os = "windows", target_arch = "x86_64")))]
fn native_machine_is_arm64(native_machine: Option<u16>) -> bool {
    native_machine == Some(IMAGE_FILE_MACHINE_ARM64)
}

pub fn is_windows_x64_emulated_on_arm64() -> bool {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        use std::sync::OnceLock;

        static DETECTED: OnceLock<bool> = OnceLock::new();
        *DETECTED.get_or_init(|| native_machine_is_arm64(native_windows_machine()))
    }

    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    {
        false
    }
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn native_windows_machine() -> Option<u16> {
    use windows::core::{s, w, BOOL};
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::Win32::System::Threading::GetCurrentProcess;

    type IsWow64Process2 = unsafe extern "system" fn(HANDLE, *mut u16, *mut u16) -> BOOL;

    unsafe {
        let kernel32 = GetModuleHandleW(w!("kernel32.dll")).ok()?;
        let address = GetProcAddress(kernel32, s!("IsWow64Process2"))?;

        let is_wow64_process2: IsWow64Process2 = std::mem::transmute(address);
        let mut process_machine = 0u16;
        let mut native_machine = 0u16;
        is_wow64_process2(
            GetCurrentProcess(),
            &mut process_machine,
            &mut native_machine,
        )
        .as_bool()
        .then_some(native_machine)
    }
}

pub fn cancel_current_operation(app: &AppHandle) {
    info!("Initiating operation cancellation...");

    shortcut::unregister_cancel_shortcut(app);

    PipelineGeneration::bump(app);

    if let Some(coordinator) = app.try_state::<TranscriptionCoordinator>() {
        coordinator.notify_cancel(true);
    }

    change_tray_icon(app, crate::tray::TrayIconState::Idle);
    hide_recording_overlay(app);

    {
        let app = app.clone();
        std::thread::spawn(move || {
            let audio_manager = app.state::<Arc<AudioRecordingManager>>();
            audio_manager.cancel_recording();

            audio_manager.remove_mute();

            let tm = app.state::<Arc<TranscriptionManager>>();
            tm.maybe_unload_immediately("cancellation");
        });
    }

    info!("Operation cancellation dispatched - UI returned to idle state");
}

#[cfg(target_os = "linux")]
pub fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|v| v.to_lowercase() == "wayland")
            .unwrap_or(false)
}

#[cfg(target_os = "linux")]
pub fn is_kde_plasma() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|v| v.to_uppercase().contains("KDE"))
        .unwrap_or(false)
        || std::env::var("KDE_SESSION_VERSION").is_ok()
}

#[cfg(target_os = "linux")]
pub fn is_kde_wayland() -> bool {
    is_wayland() && is_kde_plasma()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loggable_path_folds_the_home_directory_away() {
        let home = Path::new("/home/example-user");
        let inside = home.join("AppData").join("Roaming").join("history.db");
        let folded = loggable_path_within(&inside, Some(home));
        assert!(
            !folded.contains("example-user"),
            "the account name survived: {folded}"
        );
        assert!(folded.starts_with('~'), "{folded}");

        assert!(folded.ends_with("history.db"), "{folded}");
    }

    #[test]
    fn loggable_path_keeps_paths_outside_the_home_directory() {
        let outside = Path::new("/opt/anonen/anonen.exe");
        assert_eq!(
            loggable_path_within(outside, Some(Path::new("/home/example-user"))),
            "/opt/anonen/anonen.exe"
        );
    }

    #[test]
    fn loggable_path_falls_back_to_the_file_name_without_a_home() {
        let path = Path::new("/home/example-user/AppData/history.db");
        assert_eq!(loggable_path_within(path, None), "history.db");
    }

    #[test]
    fn a_native_log_line_loses_the_account_name_but_keeps_the_message() {
        let home = Path::new("C:\\Users\\example-user");
        let line = "whisper_init_from_file_with_params_no_state: loading model from \
                    'C:\\Users\\example-user\\AppData\\Local\\net.anonen.app\\models\\large.bin'";

        let redacted = redact_home_paths_within(line, Some(home));

        assert!(!redacted.contains("example-user"), "{redacted}");
        assert!(redacted.contains("loading model from"), "{redacted}");
        assert!(
            redacted.contains("large.bin"),
            "どのモデルかは残す: {redacted}"
        );
    }

    #[test]
    fn every_occurrence_in_one_line_is_folded() {
        let home = Path::new("/home/example-user");
        let line = "copy /home/example-user/a.bin -> /home/example-user/b.bin";

        assert_eq!(
            redact_home_paths_within(line, Some(home)),
            "copy ~/a.bin -> ~/b.bin"
        );
    }

    #[test]
    fn the_spelling_of_the_home_directory_may_differ_in_case() {
        let home = Path::new("C:\\Users\\Example-User");
        assert_eq!(
            redact_home_paths_within("open c:\\users\\example-user\\logs", Some(home)),
            "open ~\\logs"
        );
    }

    #[test]
    fn a_line_without_the_home_directory_is_left_alone() {
        let home = Path::new("/home/example-user");
        let line = "ggml_vulkan: Device memory allocation of size 1258291200 failed.";
        assert_eq!(redact_home_paths_within(line, Some(home)), line);

        assert_eq!(redact_home_paths_within(line, None), line);
    }

    #[test]
    fn a_multibyte_line_keeps_its_characters() {
        let home = Path::new("/home/example-user");
        let line = "モデルを /home/example-user/models/大きいの.bin から読み込みます";
        assert_eq!(
            redact_home_paths_within(line, Some(home)),
            "モデルを ~/models/大きいの.bin から読み込みます"
        );
    }

    #[test]
    fn loggable_path_opt_names_the_missing_case() {
        assert_eq!(loggable_path_opt(None), "<unknown>");
    }

    #[test]
    fn foreign_text_flattens_control_characters() {
        assert_eq!(
            foreign_text("あのねん\n\nお支払い情報を更新してください", 200),
            "あのねん  お支払い情報を更新してください"
        );
        assert_eq!(foreign_text("  \t padded \r\n ", 200), "padded");
    }

    #[test]
    fn foreign_text_caps_by_characters_not_bytes() {
        let long = "あ".repeat(300);
        let capped = foreign_text(&long, 200);
        assert_eq!(capped.chars().count(), 201);
        assert!(capped.ends_with('…'));
    }

    #[test]
    fn foreign_text_leaves_short_strings_alone() {
        assert_eq!(foreign_text("model not found", 200), "model not found");
    }

    #[test]
    fn notifications_are_capped_shorter_than_toasts() {
        let long = "x".repeat(500);
        assert_eq!(
            quoted_body(&long).chars().count(),
            QUOTED_BODY_MAX_CHARS + 1
        );
        assert_eq!(
            notification_reason(&long).chars().count(),
            NOTIFICATION_REASON_MAX_CHARS + 1
        );
        const { assert!(NOTIFICATION_REASON_MAX_CHARS < QUOTED_BODY_MAX_CHARS) };
    }

    #[test]
    fn arm64_native_machine_is_the_only_match() {
        assert!(native_machine_is_arm64(Some(IMAGE_FILE_MACHINE_ARM64)));
        assert!(!native_machine_is_arm64(Some(0x8664)));
        assert!(!native_machine_is_arm64(Some(0x014c)));
        assert!(!native_machine_is_arm64(None));
    }
}
