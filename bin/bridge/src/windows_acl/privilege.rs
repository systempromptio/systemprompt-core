//! Token privilege enablement for the elevated child.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr::null_mut;
use windows_sys::Win32::Security::{
    AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use super::checked;

const SE_RESTORE_NAME: &str = "SeRestorePrivilege";

pub(super) fn enable_restore() -> io::Result<()> {
    let name: Vec<u16> = SE_RESTORE_NAME.encode_utf16().chain(Some(0)).collect();
    let mut token = null_mut();
    let mut privileges = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: windows_sys::Win32::Foundation::LUID {
                LowPart: 0,
                HighPart: 0,
            },
            Attributes: SE_PRIVILEGE_ENABLED,
        }],
    };
    // SAFETY: every pointer names a live local; the token handle is owned and
    // closed on drop.
    unsafe {
        checked(OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &raw mut token,
        ))?;
        let token = OwnedHandle::from_raw_handle(token);
        checked(LookupPrivilegeValueW(
            null_mut(),
            name.as_ptr(),
            &raw mut privileges.Privileges[0].Luid,
        ))?;
        checked(AdjustTokenPrivileges(
            token.as_raw_handle(),
            0,
            &raw const privileges,
            0,
            null_mut(),
            null_mut(),
        ))?;
    }
    // Why: AdjustTokenPrivileges reports success even when the privilege is
    // absent from the token; the last error is the only signal.
    if io::Error::last_os_error().raw_os_error()
        == Some(windows_sys::Win32::Foundation::ERROR_NOT_ALL_ASSIGNED as i32)
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "SeRestorePrivilege is not held; the repair must run elevated",
        ));
    }
    Ok(())
}
