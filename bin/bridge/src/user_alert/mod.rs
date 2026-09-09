//! A modal "this needs you" dialog on the host's own UI toolkit — the one
//! thing an installer or the GUI may raise without a webview.
//!
//! Lives below `gui` so a host installer (Codex's profile-approval notice)
//! can raise it without reaching up into the window layer. The dialog is
//! raised and left for the person to dismiss; the caller never waits on it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(target_os = "windows")]
mod msgbox;

#[cfg(not(target_os = "windows"))]
use std::process::Command;

pub fn alert_user(title: &str, message: &str) {
    tracing::warn!(title = %title, message = %message, "alerting user");
    // Why: the callers are installer paths with no window and no return
    // channel, so the dialog must never hold them; a modal that nobody
    // dismisses (a CI runner, an unattended install) otherwise blocks the
    // process forever.
    #[cfg(target_os = "windows")]
    {
        let title = title.to_owned();
        let message = message.to_owned();
        std::thread::spawn(move || msgbox::show(&title, &message));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let title = title.replace(['"', '\''], "");
        let message = message.replace(['"', '\''], "");
        let spawned = std::cfg_select! {
            target_os = "macos" => Command::new("/usr/bin/osascript")
                .arg("-e")
                .arg(format!(
                    "display dialog \"{message}\" with title \"{title}\" buttons {{\"OK\"}} with icon stop"
                ))
                .spawn(),
            _ => Command::new("notify-send")
                .args(["--urgency=critical", &title, &message])
                .spawn(),
        };
        match spawned {
            Ok(mut child) => {
                std::thread::spawn(move || match child.wait() {
                    Ok(status) if !status.success() => {
                        tracing::warn!(%status, "user alert dialog exited unsuccessfully");
                    },
                    Ok(_) => {},
                    Err(e) => tracing::error!(error = %e, "failed to reap user alert dialog"),
                });
            },
            Err(e) => tracing::error!(error = %e, "failed to alert user"),
        }
    }
}
