//! Per-user singleton for the bridge GUI / proxy.
//!
//! Launching twice races on the proxy port (48217), the OAuth loopback port,
//! and the GUI server port. We hold a named OS lock for the lifetime of the
//! process; the second launcher sees `AlreadyRunning` and pings the running
//! instance over the local TCP server to focus its window.

use std::fs;
use std::io::Write as _;
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

pub(crate) enum SingletonResult {
    Acquired(SingletonGuard),
    AlreadyRunning,
    Error(String),
}

pub(crate) struct SingletonGuard {
    #[cfg(unix)]
    _file: std::fs::File,
    #[cfg(windows)]
    _handle: windows::MutexHandle,
}

#[must_use]
pub(crate) fn try_acquire_gui() -> SingletonResult {
    #[cfg(unix)]
    {
        unix::acquire()
    }
    #[cfg(windows)]
    {
        windows::acquire()
    }
    #[cfg(not(any(unix, windows)))]
    {
        SingletonResult::Acquired(SingletonGuard {})
    }
}

#[cfg(unix)]
mod unix {
    #![allow(
        unsafe_code,
        reason = "POSIX flock(2) FFI for single-instance lockfile"
    )]

    use super::{SingletonGuard, SingletonResult};
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::io::AsRawFd;

    pub(super) fn acquire() -> SingletonResult {
        let path = match super::lock_path() {
            Ok(p) => p,
            Err(e) => return SingletonResult::Error(e),
        };
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return SingletonResult::Error(format!(
                    "create lock dir {}: {e}",
                    parent.display()
                ));
            }
        }
        let mut file = match OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)
        {
            Ok(f) => f,
            Err(e) => {
                return SingletonResult::Error(format!("open lock {}: {e}", path.display()));
            },
        };
        let fd = file.as_raw_fd();
        let rc = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
        if rc != 0 {
            let err = std::io::Error::last_os_error();
            return match err.raw_os_error() {
                Some(libc::EWOULDBLOCK) => SingletonResult::AlreadyRunning,
                _ => SingletonResult::Error(format!("flock {}: {err}", path.display())),
            };
        }
        _ = file.set_len(0);
        _ = writeln!(file, "{}", std::process::id());
        SingletonResult::Acquired(SingletonGuard { _file: file })
    }
}

#[cfg(windows)]
mod windows {
    #![allow(
        unsafe_code,
        reason = "Win32 named-mutex FFI for single-instance enforcement"
    )]

    use super::{SingletonGuard, SingletonResult};
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
    use windows_sys::Win32::System::Threading::CreateMutexW;

    pub(super) struct MutexHandle(HANDLE);

    impl Drop for MutexHandle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CloseHandle(self.0) };
            }
        }
    }

    pub(super) fn acquire() -> SingletonResult {
        let name: Vec<u16> = "Local\\SystempromptBridgeSingleton\0"
            .encode_utf16()
            .collect();
        let handle = unsafe { CreateMutexW(std::ptr::null(), 1, name.as_ptr()) };
        if handle.is_null() {
            let err = unsafe { GetLastError() };
            return SingletonResult::Error(format!("CreateMutexW failed: {err}"));
        }
        let last_error = unsafe { GetLastError() };
        if last_error == ERROR_ALREADY_EXISTS {
            unsafe { CloseHandle(handle) };
            return SingletonResult::AlreadyRunning;
        }
        SingletonResult::Acquired(SingletonGuard {
            _handle: MutexHandle(handle),
        })
    }
}

#[cfg(unix)]
pub fn lock_path() -> Result<PathBuf, String> {
    let base = dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir);
    Ok(base.join("systemprompt").join("bridge.lock"))
}

#[cfg(windows)]
#[expect(
    clippy::unnecessary_wraps,
    reason = "mirrors the fallible unix lock_path so the shared sidecar_path call site stays cfg-agnostic"
)]
pub(crate) fn lock_path() -> Result<PathBuf, String> {
    let base = dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir);
    Ok(base.join("systemprompt").join("bridge.lock"))
}

fn sidecar_path() -> Option<PathBuf> {
    #[cfg(any(unix, windows))]
    {
        lock_path().ok().map(|p| p.with_extension("json"))
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

pub(crate) fn write_running_port(port: u16, csrf_token: &str) {
    let Some(path) = sidecar_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        _ = fs::create_dir_all(parent);
    }
    let payload = serde_json::json!({
        "pid": std::process::id(),
        "port": port,
        "token": csrf_token,
    });
    if let Ok(mut f) = fs::File::create(&path) {
        _ = f.write_all(payload.to_string().as_bytes());
    }
}

pub(crate) fn clear_running_port() {
    let Some(path) = sidecar_path() else {
        return;
    };
    _ = fs::remove_file(path);
}

#[derive(Debug, Clone)]
struct RunningInstance {
    port: u16,
    token: String,
}

#[must_use]
fn read_running_instance() -> Option<RunningInstance> {
    let path = sidecar_path()?;
    let raw = fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let port = u16::try_from(v.get("port")?.as_u64()?).ok()?;
    let token = v.get("token")?.as_str()?.to_string();
    Some(RunningInstance { port, token })
}

pub(crate) fn ping_focus_running_instance() -> bool {
    let Some(instance) = read_running_instance() else {
        return false;
    };
    let addr = format!("127.0.0.1:{}", instance.port);
    let Ok(parsed) = addr.parse() else {
        return false;
    };
    let Ok(stream) = TcpStream::connect_timeout(&parsed, Duration::from_millis(250)) else {
        return false;
    };
    _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
    _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let mut stream = stream;
    let body = "";
    let request = format!(
        "POST /api/focus_window?t={} HTTP/1.1\r\nHost: localhost\r\nContent-Length: \
         {}\r\nConnection: close\r\n\r\n{}",
        instance.token,
        body.len(),
        body,
    );
    stream.write_all(request.as_bytes()).is_ok()
}
