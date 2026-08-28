//! Desktop Window Manager attributes for the settings window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]
#![allow(unsafe_code, reason = "Win32 dwmapi window-attribute FFI")]

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use winit::window::Window;

pub fn set_immersive_dark(window: &dyn Window, dark: bool) {
    let Some(hwnd) = hwnd_of(window) else {
        return;
    };
    let value: i32 = i32::from(dark);
    // SAFETY: `hwnd` is a live HWND owned by the caller's window, and the
    // attribute buffer is a `BOOL`-sized i32 on this frame. DWM only reads it.
    let hr = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            std::ptr::from_ref(&value).cast(),
            u32::try_from(size_of::<i32>()).unwrap_or(4),
        )
    };
    if hr < 0 {
        // Why: unsupported before Windows 10 1809, where the correct behaviour
        // is a light title bar. Not an error worth surfacing.
        tracing::debug!(hr, "DWMWA_USE_IMMERSIVE_DARK_MODE rejected");
    }
}

fn hwnd_of(window: &dyn Window) -> Option<HWND> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(w) => Some(w.hwnd.get() as HWND),
        _ => None,
    }
}
