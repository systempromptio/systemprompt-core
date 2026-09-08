//! Per-user singleton for the bridge GUI / proxy.
//!
//! A second launch would race the proxy/loopback/GUI ports, so we hold a named
//! OS lock; the loser pings the running instance to focus its window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::io::{Read as _, Write as _};
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
    _file: fs::File,
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
        let path = super::lock_path();
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            return SingletonResult::Error(format!("create lock dir {}: {e}", parent.display()));
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
        // SAFETY: `fd` is a valid open descriptor owned by `file`, which outlives this
        // call.
        let rc = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
        if rc != 0 {
            let err = std::io::Error::last_os_error();
            return match err.raw_os_error() {
                Some(libc::EWOULDBLOCK) => SingletonResult::AlreadyRunning,
                _ => SingletonResult::Error(format!("flock {}: {err}", path.display())),
            };
        }
        if let Err(e) = file
            .set_len(0)
            .and_then(|()| writeln!(file, "{}", std::process::id()))
            .and_then(|()| file.sync_all())
        {
            return SingletonResult::Error(format!("write lock {}: {e}", path.display()));
        }
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
                // SAFETY: `self.0` is a non-null mutex handle this guard exclusively owns.
                unsafe { CloseHandle(self.0) };
            }
        }
    }

    pub(super) fn acquire() -> SingletonResult {
        let name: Vec<u16> = "Local\\SystempromptBridgeSingleton\0"
            .encode_utf16()
            .collect();
        // SAFETY: `name` is a live NUL-terminated UTF-16 buffer; a null security
        // descriptor is valid and requests default access.
        let handle = unsafe { CreateMutexW(std::ptr::null(), 1, name.as_ptr()) };
        if handle.is_null() {
            // SAFETY: `GetLastError` reads thread-local error state with no preconditions.
            let err = unsafe { GetLastError() };
            return SingletonResult::Error(format!("CreateMutexW failed: {err}"));
        }
        // SAFETY: `GetLastError` reads thread-local error state with no preconditions.
        let last_error = unsafe { GetLastError() };
        if last_error == ERROR_ALREADY_EXISTS {
            // SAFETY: `handle` is the non-null mutex handle just returned by
            // `CreateMutexW`.
            unsafe { CloseHandle(handle) };
            return SingletonResult::AlreadyRunning;
        }
        SingletonResult::Acquired(SingletonGuard {
            _handle: MutexHandle(handle),
        })
    }
}

#[cfg(unix)]
pub(crate) fn lock_path() -> PathBuf {
    let base = crate::basedirs::data_local_dir()
        .or_else(crate::basedirs::home_dir)
        .unwrap_or_else(std::env::temp_dir);
    base.join(crate::brand::brand().config_dir)
        .join("bridge.lock")
}

#[cfg(windows)]
pub(crate) fn lock_path() -> PathBuf {
    let base = crate::basedirs::data_local_dir()
        .or_else(crate::basedirs::home_dir)
        .unwrap_or_else(std::env::temp_dir);
    base.join(crate::brand::brand().config_dir)
        .join("bridge.lock")
}

fn sidecar_path() -> PathBuf {
    #[cfg(any(unix, windows))]
    {
        lock_path().with_extension("json")
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

pub(crate) fn write_running_port(port: u16, csrf_token: &str) -> std::io::Result<()> {
    let path = sidecar_path();
    let payload =
        serde_json::json!({ "pid": std::process::id(), "port": port, "token": csrf_token });
    crate::fsutil::atomic_write_0600(&path, payload.to_string().as_bytes())
}

pub(crate) fn clear_running_port() -> std::io::Result<()> {
    let path = sidecar_path();
    match fs::remove_file(&path) {
        Ok(()) => {},
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => return Err(e),
    }
    if path.try_exists()? {
        return Err(std::io::Error::other(format!(
            "{}: sidecar removal did not land",
            path.display()
        )));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct RunningInstance {
    port: u16,
    token: String,
}

#[must_use]
fn read_running_instance() -> Option<RunningInstance> {
    let path = sidecar_path();
    let raw = fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let port = u16::try_from(v.get("port")?.as_u64()?).ok()?;
    let token = v.get("token")?.as_str()?.to_owned();
    Some(RunningInstance { port, token })
}

pub(crate) fn ping_focus_running_instance() -> bool {
    let Some(instance) = read_running_instance() else {
        return false;
    };
    if focus_handshake(&instance) {
        return true;
    }
    if let Err(e) = clear_running_port() {
        crate::stdio::diag(&format!("remove stale bridge sidecar: {e}"));
    }
    false
}

fn focus_handshake(instance: &RunningInstance) -> bool {
    let Ok(parsed) = format!("127.0.0.1:{}", instance.port).parse() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&parsed, Duration::from_millis(250)) else {
        return false;
    };
    if stream
        .set_write_timeout(Some(Duration::from_millis(250)))
        .is_err()
        || stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .is_err()
    {
        return false;
    }
    let request = format!(
        "POST /api/focus_window?t={} HTTP/1.1\r\nHost: localhost\r\nContent-Length: \
         0\r\nConnection: close\r\n\r\n",
        instance.token,
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut buf = [0u8; 16];
    let mut filled = 0;
    while filled < buf.len() {
        match stream.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled = filled.saturating_add(n),
            Err(_) => return false,
        }
    }
    buf.get(..filled)
        .is_some_and(|got| got.starts_with(b"HTTP/1.1 204"))
}
