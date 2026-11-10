//! The owner SID of an open request file, read from its security descriptor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]
#![expect(unsafe_code, reason = "the owner of an open handle is only reachable through the Win32 security API")]

use std::fs::File;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::ptr::null_mut;

use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, GetSecurityInfo, SE_FILE_OBJECT,
};
use windows_sys::Win32::Security::{OWNER_SECURITY_INFORMATION, PSID};

use crate::windows_acl::Descriptor;

pub(super) fn owner_of(file: &File) -> io::Result<String> {
    let mut owner: PSID = null_mut();
    let mut descriptor = null_mut();
    // SAFETY: the handle is open with READ_CONTROL and the output slots are
    // live; the descriptor Windows allocates owns the SID for as long as the
    // `Descriptor` guard lives, and the SID string is freed by its own guard.
    unsafe {
        let status = GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &raw mut owner,
            null_mut(),
            null_mut(),
            null_mut(),
            &raw mut descriptor,
        );
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        let _descriptor = Descriptor(descriptor);
        if owner.is_null() {
            return Err(io::Error::other("request file has no owner"));
        }
        let mut text = null_mut();
        if ConvertSidToStringSidW(owner, &raw mut text) == 0 {
            return Err(io::Error::last_os_error());
        }
        let _text = Descriptor(text.cast());
        let mut n = 0;
        while *text.add(n) != 0 {
            n += 1;
        }
        String::from_utf16(std::slice::from_raw_parts(text, n)).map_err(io::Error::other)
    }
}
