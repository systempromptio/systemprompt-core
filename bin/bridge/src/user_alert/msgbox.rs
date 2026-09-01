//! Native modal alert box for failures that happen before, or instead of, a
//! window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]
#![allow(unsafe_code, reason = "Win32 user32 MessageBoxW FFI")]

use windows_sys::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MB_SYSTEMMODAL, MessageBoxW,
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(super) fn show(title: &str, message: &str) {
    let title = wide(title);
    let message = wide(message);
    // SAFETY: both buffers are NUL-terminated UTF-16 owned by this frame and
    // outlive the call; a null HWND makes the box ownerless, which is what we
    // want when there is no window yet.
    unsafe {
        _ = MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND | MB_SYSTEMMODAL,
        );
    }
}
