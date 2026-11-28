//! Shared plumbing for the Win32 calls: ownership of `LocalAlloc` memory,
//! translation of BOOL and status returns into `io::Error`, and NUL-terminated
//! wide paths.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![expect(unsafe_code, reason = "LocalFree is raw Win32 FFI")]

use std::ffi::c_void;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows_sys::Win32::Foundation::LocalFree;

pub(super) struct LocalAllocation(pub(super) *mut c_void);

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        // SAFETY: every LocalAllocation wraps memory a Win32 API allocated and
        // documents as freed with LocalFree; it is freed exactly once, here.
        unsafe {
            LocalFree(self.0);
        }
    }
}

pub(super) fn checked(value: i32) -> io::Result<()> {
    if value == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(super) fn status(value: u32) -> io::Result<()> {
    if value == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(value.cast_signed()))
    }
}

pub(super) fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let mut value: Vec<u16> = path.as_os_str().encode_wide().collect();
    if value.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains NUL",
        ));
    }
    value.push(0);
    Ok(value)
}
