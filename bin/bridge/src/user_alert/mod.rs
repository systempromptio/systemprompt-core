//! A blocking, modal "this needs you" dialog on the host's own UI toolkit —
//! the one thing an installer or the GUI may raise without a webview.
//!
//! Lives below `gui` so a host installer (Codex's profile-approval notice)
//! can raise it without reaching up into the window layer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(target_os = "windows")]
mod msgbox;

#[cfg(not(target_os = "windows"))]
use std::process::Command;

pub fn alert_user(title: &str, message: &str) {
    tracing::warn!(title = %title, message = %message, "alerting user");
    #[cfg(target_os = "windows")]
    {
        msgbox::show(title, message);
    }
    #[cfg(not(target_os = "windows"))]
    {
        // Why: the shells below are quote-delimited; dropping quotes from the
        // interpolated text keeps the command well-formed without an escaper.
        let title = title.replace(['"', '\''], "");
        let message = message.replace(['"', '\''], "");
        let spawned = std::cfg_select! {
            target_os = "macos" => Command::new("/usr/bin/osascript")
                .arg("-e")
                .arg(format!(
                    "display dialog \"{message}\" with title \"{title}\" buttons {{\"OK\"}} with icon stop"
                ))
                .status(),
            _ => Command::new("notify-send")
                .args(["--urgency=critical", &title, &message])
                .status(),
        };
        if let Err(e) = spawned {
            tracing::error!(error = %e, "failed to alert user");
        }
    }
}
