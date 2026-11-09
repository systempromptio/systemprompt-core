//! Per-user singleton for the bridge GUI / proxy.
//!
//! A second launch would race the proxy/loopback/GUI ports, so we hold a named
//! OS lock. The loser reads the running instance's sidecar: a newer build asks
//! it to hand over and waits for the lock, anything else asks it to focus its
//! window (see [`crate::takeover`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::PathBuf;

mod control;
mod error;
mod handoff;

pub(crate) use error::SingletonError;
#[cfg(unix)]
use error::lock_error;

pub(crate) use control::{ping_focus_running_instance, take_over_older_instance};
pub(crate) use handoff::{await_predecessor_exit, record_handoff};

pub(crate) enum SingletonResult {
    Acquired(SingletonGuard),
    AlreadyRunning,
    Error(SingletonError),
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
            return super::lock_error("create lock dir", parent, e);
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
                return super::lock_error("open lock", &path, e);
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
                _ => super::lock_error("flock", &path, err),
            };
        }
        if let Err(e) = file
            .set_len(0)
            .and_then(|()| writeln!(file, "{}", std::process::id()))
            .and_then(|()| file.sync_all())
        {
            return super::lock_error("write lock", &path, e);
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
            return SingletonResult::Error(super::SingletonError::Mutex { status: err });
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct RunningSidecar {
    pub(super) pid: u32,
    pub(super) port: u16,
    pub(super) token: String,
    #[serde(default)]
    pub(super) version: Option<String>,
}

pub(crate) fn write_running_port(port: u16, csrf_token: &str) -> std::io::Result<()> {
    let path = sidecar_path();
    let payload = RunningSidecar {
        pid: std::process::id(),
        port,
        token: csrf_token.to_owned(),
        version: Some(crate::brand::brand().version.to_owned()),
    };
    let bytes = serde_json::to_vec(&payload)?;
    crate::fsutil::atomic_write_0600(&path, &bytes)
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

pub(super) fn read_running_instance() -> Option<RunningSidecar> {
    let path = sidecar_path();
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "running-instance sidecar unreadable");
            return None;
        },
    };
    match serde_json::from_str::<RunningSidecar>(&raw) {
        Ok(sidecar) => Some(sidecar),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "running-instance sidecar malformed");
            None
        },
    }
}
