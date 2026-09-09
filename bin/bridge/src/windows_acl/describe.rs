//! Owner and DACL of a path as SDDL, for doctor and the diagnostics bundle
//! to show why a private file stopped being readable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::Path;
use std::ptr::null_mut;
use windows_sys::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW, SDDL_REVISION_1,
    SE_FILE_OBJECT,
};
use windows_sys::Win32::Security::{DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION};

use super::{Descriptor, checked, current_sid, status, wide};

pub(crate) fn describe(path: &Path) -> io::Result<String> {
    let wide_path = wide(path)?;
    let mut descriptor = null_mut();
    // SAFETY: the path is NUL terminated and the descriptor slot is a live local;
    // Windows allocates the descriptor, freed by `Descriptor`.
    unsafe {
        status(GetNamedSecurityInfoW(
            wide_path.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            &raw mut descriptor,
        ))?;
    }
    let descriptor = Descriptor(descriptor);
    let mut text = null_mut();
    let mut length = 0u32;
    // SAFETY: the descriptor is live and self-relative; the string is allocated
    // by Windows and freed with LocalFree below.
    unsafe {
        checked(ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor.0,
            SDDL_REVISION_1,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &raw mut text,
            &raw mut length,
        ))?;
        let allocation = Descriptor(text.cast());
        let mut n = 0;
        while *text.add(n) != 0 {
            n += 1;
        }
        let sddl =
            String::from_utf16(std::slice::from_raw_parts(text, n)).map_err(io::Error::other);
        drop(allocation);
        Ok(format!("sddl {} (this process: {})", sddl?, current_sid()?))
    }
}
