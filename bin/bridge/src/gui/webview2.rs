//! First-launch guard for the `WebView2` Evergreen runtime.
//!
//! A `windows_subsystem = "windows"` binary has no console to report on and no
//! window to report in, so a failed webview creation means the app simply never
//! appears. The presence check itself lives in `crate::webview2` so diagnostics
//! can report it without reaching into the GUI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

use crate::webview2::{BOOTSTRAPPER_URL, runtime_version};

#[must_use]
pub fn ensure_present() -> bool {
    if let Some(version) = runtime_version() {
        tracing::info!(%version, "webview2 runtime present");
        return true;
    }
    let app = crate::brand::brand().app_name;
    crate::user_alert::alert_user(
        &format!("{app} needs the Microsoft WebView2 runtime"),
        "The Evergreen WebView2 runtime is not installed on this machine, so the app cannot \
         render its window. The download page will open now; install it and start the app again.",
    );
    crate::gui::window::open_external_url(BOOTSTRAPPER_URL);
    false
}
