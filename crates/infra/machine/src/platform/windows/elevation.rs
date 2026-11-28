//! The elevation of the process token, and a program started through the UAC
//! consent dialog.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![expect(
    unsafe_code,
    reason = "the process token and the elevated launch are raw Win32 FFI"
)]

use std::ffi::OsStr;
use std::io;
use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle};
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{ERROR_CANCELLED, WAIT_OBJECT_0};
use windows_sys::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetExitCodeProcess, INFINITE, OpenProcessToken, WaitForSingleObject,
};
use windows_sys::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

use super::ffi::wide;
use crate::elevation::{ElevatedCommand, Elevation, ElevationError, ElevationOutcome};
use crate::quote;

// Why: the shell starts a program through the consent dialog only for this
// verb.
const ELEVATE_VERB: &str = "runas";

pub(super) fn current() -> io::Result<Elevation> {
    let mut token = null_mut();
    // SAFETY: the output points at a live handle slot; on success the token is
    // owned by the OwnedHandle and closed on drop.
    let token = unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        OwnedHandle::from_raw_handle(token)
    };
    let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut returned = 0u32;
    // SAFETY: the token is a live TOKEN_QUERY handle and the buffer is a
    // TOKEN_ELEVATION whose size is the length passed.
    let read = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenElevation,
            (&raw mut elevation).cast(),
            size_of::<TOKEN_ELEVATION>() as u32,
            &raw mut returned,
        )
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(if elevation.TokenIsElevated == 0 {
        Elevation::Standard
    } else {
        Elevation::Elevated
    })
}

fn wait(process: &OwnedHandle) -> io::Result<u32> {
    let mut code = 0u32;
    // SAFETY: the handle is a live process handle this function borrows, and
    // the exit code slot is a live local.
    unsafe {
        if WaitForSingleObject(process.as_raw_handle(), INFINITE) != WAIT_OBJECT_0
            || GetExitCodeProcess(process.as_raw_handle(), &raw mut code) == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(code)
}

pub(super) fn elevate(command: &ElevatedCommand) -> Result<ElevationOutcome, ElevationError> {
    let verb = wide(OsStr::new(ELEVATE_VERB));
    let file = wide(OsStr::new(command.program()));
    let parameters = wide(OsStr::new(&quote::windows_command_line(
        command.arguments(),
    )));
    let mut launch = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: verb.as_ptr(),
        lpFile: file.as_ptr(),
        lpParameters: parameters.as_ptr(),
        nShow: SW_HIDE,
        ..Default::default()
    };
    // SAFETY: the structure carries its own size and every string it points at
    // is a NUL-terminated buffer that outlives the call.
    if unsafe { ShellExecuteExW(&raw mut launch) } == 0 {
        let source = io::Error::last_os_error();
        return if source.raw_os_error() == Some(ERROR_CANCELLED.cast_signed()) {
            Ok(ElevationOutcome::Declined)
        } else {
            Err(ElevationError::Spawn {
                command: command.program().to_owned(),
                source,
            })
        };
    }
    if launch.hProcess.is_null() {
        return Err(ElevationError::Wait {
            source: io::Error::other("the elevated launch returned no process"),
        });
    }
    // SAFETY: with SEE_MASK_NOCLOSEPROCESS a successful launch hands the caller
    // a process handle to close; the OwnedHandle closes it once.
    let process = unsafe { OwnedHandle::from_raw_handle(launch.hProcess) };
    let code = wait(&process).map_err(|source| ElevationError::Wait { source })?;
    Ok(if code == 0 {
        ElevationOutcome::Succeeded
    } else {
        ElevationOutcome::Failed {
            code: Some(code.cast_signed()),
            detail: String::new(),
        }
    })
}
