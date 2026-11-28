//! Shared plumbing for the Win32 calls: NUL-terminated wide text, the system
//! directory and the machine's `ProgramData`, both asked of the system instead
//! of read from the environment a user controls.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![expect(
    unsafe_code,
    reason = "the system directory and the known folder are raw Win32 FFI"
)]

use std::ffi::{OsStr, OsString, c_void};
use std::io;
use std::os::windows::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::PathBuf;
use std::ptr::null_mut;

use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows_sys::Win32::UI::Shell::{FOLDERID_ProgramData, KF_FLAG_DEFAULT, SHGetKnownFolderPath};

const SYSTEM_DIRECTORY_CAPACITY: usize = 260;

pub(super) fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

struct TaskAllocation(*mut c_void);

impl Drop for TaskAllocation {
    fn drop(&mut self) {
        // SAFETY: the pointer is null or memory the shell allocated for the
        // caller to release with CoTaskMemFree; it is released exactly once.
        unsafe {
            CoTaskMemFree(self.0);
        }
    }
}

pub(super) fn system_directory() -> io::Result<PathBuf> {
    let mut buffer = [0u16; SYSTEM_DIRECTORY_CAPACITY];
    // SAFETY: the buffer is a live local and the length passed is its capacity
    // in characters.
    let length =
        unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), SYSTEM_DIRECTORY_CAPACITY as u32) };
    match buffer.get(..length as usize) {
        Some(units) if !units.is_empty() => Ok(PathBuf::from(OsString::from_wide(units))),
        _ => Err(io::Error::last_os_error()),
    }
}

pub(super) fn program_data() -> io::Result<PathBuf> {
    let mut text = null_mut();
    // SAFETY: the folder id is a static GUID and the output slot is a live
    // local; whatever the shell stores there is owned by the TaskAllocation.
    let result = unsafe {
        SHGetKnownFolderPath(
            &FOLDERID_ProgramData,
            KF_FLAG_DEFAULT as u32,
            null_mut(),
            &raw mut text,
        )
    };
    let allocation = TaskAllocation(text.cast());
    if result < 0 || text.is_null() {
        return Err(io::Error::from_raw_os_error(result));
    }
    // SAFETY: on success the shell stored a NUL-terminated wide string, which
    // stays allocated until `allocation` drops below.
    let path = unsafe {
        let mut length = 0;
        while *text.add(length) != 0 {
            length += 1;
        }
        PathBuf::from(OsString::from_wide(std::slice::from_raw_parts(
            text, length,
        )))
    };
    drop(allocation);
    Ok(path)
}
