//! Windows private-file descriptors and effective access verification.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![allow(unsafe_code, reason = "Windows descriptor and token APIs require FFI")]

mod access;
mod describe;
mod private;
mod repair;

pub(crate) use self::access::{elevation_summary, verify_modify_tree};
pub(crate) use self::describe::describe;
pub(crate) use self::private::{create_private, protect_directory, verify_private};
pub(crate) use self::repair::repair_private;

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::{
    GetTokenInformation, PSECURITY_DESCRIPTOR, TOKEN_DUPLICATE, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub(crate) struct Descriptor(pub(crate) PSECURITY_DESCRIPTOR);
impl Drop for Descriptor {
    fn drop(&mut self) {
        // SAFETY: this allocation came from an API requiring LocalFree.
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
        return Err(io::Error::other("path contains NUL"));
    }
    value.push(0);
    Ok(value)
}
pub(super) fn process_token() -> io::Result<OwnedHandle> {
    let mut token = null_mut();
    // SAFETY: output points to a live handle slot; the returned token is owned.
    unsafe {
        checked(OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY | TOKEN_DUPLICATE,
            &raw mut token,
        ))?;
        Ok(OwnedHandle::from_raw_handle(token))
    }
}
pub(crate) fn current_sid() -> io::Result<String> {
    let token = process_token()?;
    let mut length = 0;
    // SAFETY: the first query asks for the required size with no output buffer.
    let sized = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            null_mut(),
            0,
            &raw mut length,
        )
    };
    if sized != 0
        || io::Error::last_os_error().raw_os_error()
            != Some(windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER as i32)
        || length == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut storage = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
    let mut text = null_mut();
    // SAFETY: storage is aligned and sized for TOKEN_USER; SID remains live until
    // conversion ends.
    unsafe {
        checked(GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            storage.as_mut_ptr().cast(),
            length,
            &raw mut length,
        ))?;
        let user = &*storage.as_ptr().cast::<TOKEN_USER>();
        checked(ConvertSidToStringSidW(user.User.Sid, &raw mut text))?;
        let allocation = Descriptor(text.cast());
        let mut n = 0;
        while *text.add(n) != 0 {
            n += 1;
        }
        let result =
            String::from_utf16(std::slice::from_raw_parts(text, n)).map_err(io::Error::other);
        drop(allocation);
        result
    }
}
