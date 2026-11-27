//! The identity of the current user, read from the process token as a string
//! SID.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![expect(
    unsafe_code,
    reason = "the process token and SID conversion are raw Win32 FFI"
)]

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use super::ffi::{LocalAllocation, checked};

fn process_token() -> io::Result<OwnedHandle> {
    let mut token = null_mut();
    // SAFETY: the output points at a live handle slot; on success the token handle
    // is owned by the OwnedHandle and closed on drop.
    unsafe {
        checked(OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY,
            &raw mut token,
        ))?;
        Ok(OwnedHandle::from_raw_handle(token))
    }
}

pub(crate) fn current_sid() -> io::Result<String> {
    let token = process_token()?;
    let mut length = 0u32;
    // SAFETY: a size query with no output buffer; Windows only writes `length`.
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
            != Some(ERROR_INSUFFICIENT_BUFFER.cast_signed())
        || length == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut storage = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
    let mut text = null_mut();
    // SAFETY: `storage` is pointer-aligned and at least `length` bytes, as
    // TOKEN_USER needs; the SID it points into lives in `storage` until the
    // conversion returns. The string Windows allocates is freed by
    // LocalAllocation after it is copied.
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
        let allocation = LocalAllocation(text.cast());
        let mut n = 0;
        while *text.add(n) != 0 {
            n += 1;
        }
        let sid = String::from_utf16(std::slice::from_raw_parts(text, n))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
        drop(allocation);
        sid
    }
}
