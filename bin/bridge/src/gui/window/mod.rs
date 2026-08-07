//! Webview window management and external-target opening.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod native;

use std::path::Path;
use std::process::Command;

pub use native::SettingsWindow;

pub fn open_path(path: &Path) {
    open_target(&path.to_string_lossy());
}

pub fn open_external_url(url: &str) {
    open_target(url);
}

/// Tell the user something on a path that has no terminal and no window.
///
/// Why: a bundle launched from Finder/Explorer discards stderr, so a failure
/// that exits before any window exists is otherwise completely silent — the app
/// just appears not to start.
///
/// macOS deliberately uses a notification, not `display alert`. An alert is
/// drawn by the *current application*, and `osascript` spawned from a process
/// with no UI session has none: the alert never renders and `osascript` blocks
/// forever, which would leave an invisible hung process behind — strictly worse
/// than exiting. `display notification` is posted by Notification Center
/// instead, and returns immediately.
pub fn notify_user(title: &str, message: &str) {
    // Why: both shells below are quote-delimited; dropping quotes from the
    // interpolated text keeps the command well-formed without an escaper.
    let title = title.replace(['"', '\''], "");
    let message = message.replace(['"', '\''], "");
    tracing::warn!(title = %title, message = %message, "notifying user");
    let spawned = std::cfg_select! {
        target_os = "macos" => Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(format!(
                "display notification \"{message}\" with title \"{title}\""
            ))
            .status(),
        target_os = "windows" => Command::new("powershell")
            .args(["-NoProfile", "-Command"])
            .arg(format!(
                "Add-Type -AssemblyName System.Windows.Forms; \
                 [System.Windows.Forms.MessageBox]::Show('{message}', '{title}')"
            ))
            .status(),
        _ => Command::new("notify-send")
            .args(["--urgency=critical", &title, &message])
            .status(),
    };
    if let Err(e) = spawned {
        tracing::error!(error = %e, "failed to notify user");
    }
}

fn open_target(target: &str) {
    let program = std::cfg_select! {
        target_os = "macos"   => "/usr/bin/open",
        target_os = "windows" => "cmd",
        _                     => "xdg-open",
    };
    let prefix: &[&str] = std::cfg_select! {
        target_os = "windows" => &["/C", "start", ""],
        _                     => &[],
    };
    tracing::info!(target = %target, program, "opening external target");
    match Command::new(program).args(prefix).arg(target).spawn() {
        Ok(_) => {},
        Err(e) => tracing::error!(target = %target, program, error = %e, "failed to spawn opener"),
    }
}
