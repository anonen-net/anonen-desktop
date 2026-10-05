use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const NATIVE_STDERR_FILE: &str = "native-stderr.log";

const ROTATED_FILE: &str = "native-stderr.1.log";

const MAX_BYTES: u64 = 1024 * 1024;

pub fn prepare_file(log_dir: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(log_dir)?;
    let path = log_dir.join(NATIVE_STDERR_FILE);
    if fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > MAX_BYTES {
        let _ = fs::rename(&path, log_dir.join(ROTATED_FILE));
    }
    Ok(path)
}

pub fn capture_process_stderr(log_dir: &Path) -> Option<PathBuf> {
    let path = prepare_file(log_dir).ok()?;
    if redirect_stderr(&path) {
        Some(path)
    } else {
        None
    }
}

#[cfg(windows)]
fn redirect_stderr(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::IntoRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Console::{SetStdHandle, STD_ERROR_HANDLE};

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let redirected = unsafe {
        let fd = libc::wopen(
            wide.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_APPEND | libc::O_BINARY | libc::O_NOINHERIT,
            libc::S_IREAD | libc::S_IWRITE,
        );
        if fd < 0 {
            return false;
        }
        let ok = libc::dup2(fd, 2) >= 0;
        libc::close(fd);
        ok
    };
    if !redirected {
        return false;
    }

    if let Ok(file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let handle = file.into_raw_handle();

        unsafe {
            let _ = SetStdHandle(STD_ERROR_HANDLE, HANDLE(handle));
        }
    }

    true
}

#[cfg(not(windows))]
fn redirect_stderr(_path: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_bytes(path: &Path, n: usize) {
        fs::write(path, vec![b'x'; n]).unwrap();
    }

    #[test]
    fn a_small_file_is_kept_so_the_last_crash_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(NATIVE_STDERR_FILE);
        write_bytes(&path, 32);

        assert_eq!(path, prepare_file(dir.path()).unwrap());
        assert_eq!(32, fs::metadata(&path).unwrap().len());
        assert!(!dir.path().join(ROTATED_FILE).exists());
    }

    #[test]
    fn an_oversized_file_is_rotated_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(NATIVE_STDERR_FILE);
        write_bytes(&path, (MAX_BYTES + 1) as usize);

        prepare_file(dir.path()).unwrap();

        assert!(
            !path.exists(),
            "回したあとの現行ファイルは無い（次の追記で作る）"
        );
        assert_eq!(
            MAX_BYTES + 1,
            fs::metadata(dir.path().join(ROTATED_FILE)).unwrap().len()
        );
    }

    #[test]
    fn rotating_twice_keeps_only_one_generation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(NATIVE_STDERR_FILE);

        write_bytes(&path, (MAX_BYTES + 1) as usize);
        prepare_file(dir.path()).unwrap();
        write_bytes(&path, (MAX_BYTES + 2) as usize);
        prepare_file(dir.path()).unwrap();

        assert_eq!(
            MAX_BYTES + 2,
            fs::metadata(dir.path().join(ROTATED_FILE)).unwrap().len(),
            "1 世代前は新しいほうで置き換わる"
        );
        assert!(!dir.path().join("native-stderr.2.log").exists());
    }

    #[test]
    fn a_missing_log_directory_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("logs");

        let path = prepare_file(&nested).unwrap();

        assert!(nested.is_dir());
        assert_eq!(nested.join(NATIVE_STDERR_FILE), path);
    }
}
