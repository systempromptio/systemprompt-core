//! Presence check for the `WebView2` Evergreen runtime.
//!
//! A `windows_subsystem = "windows"` binary has no console to report on and no
//! window to report in, so a failed webview creation means the app simply never
//! appears. On Windows Server, stripped images and older LTSC builds the
//! runtime is absent, which makes that the likeliest way a first launch fails.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

pub const BOOTSTRAPPER_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

// Why: a 64-bit WebView2 runtime can register itself in the 32-bit registry
// view.
const CLIENT_KEY: &str =
    r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
const CLIENT_KEY_NATIVE: &str =
    r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

#[must_use]
pub fn runtime_version() -> Option<String> {
    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        for subkey in [CLIENT_KEY, CLIENT_KEY_NATIVE] {
            let found = crate::config::store::read_registry_string(hive, subkey, "pv")
                .ok()
                .flatten()
                .filter(|v| !v.trim().is_empty() && v.trim() != "0.0.0.0");
            if found.is_some() {
                return found;
            }
        }
    }
    None
}
