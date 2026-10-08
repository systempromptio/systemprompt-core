//! A modal "this needs you" dialog on the host's own UI toolkit — the one
//! thing an installer or the GUI may raise without a webview.
//!
//! Lives below `gui` so a host installer (Codex's profile-approval notice)
//! can raise it without reaching up into the window layer.
//!
//! On macOS and Linux the dialog is a separate program started through a
//! `/bin/sh` that backgrounds it and exits at once: the caller waits only for
//! that shell, the dialog stays up until the person dismisses it, and the
//! init process reaps it — no thread of ours outlives the call. On Windows
//! the box is in-process and shown on the caller's thread: every Windows
//! caller is a start-up failure about to exit, and a box on a detached thread
//! died with the process before anyone could read it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(target_os = "windows")]
mod msgbox;

#[cfg(not(target_os = "windows"))]
use std::process::{Command, Stdio};

#[cfg(not(target_os = "windows"))]
const DETACH_SCRIPT: &str = "\"$@\" </dev/null >/dev/null 2>&1 &";

pub fn alert_user(title: &str, message: &str) {
    tracing::warn!(title = %title, message = %message, "alerting user");
    #[cfg(target_os = "windows")]
    msgbox::show(title, message);
    #[cfg(not(target_os = "windows"))]
    {
        match alert_command(title, message).status() {
            Ok(status) if status.success() => {},
            Ok(status) => tracing::error!(%status, "user alert launcher exited unsuccessfully"),
            Err(e) => tracing::error!(error = %e, "failed to alert user"),
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[must_use]
pub fn alert_command(title: &str, message: &str) -> Command {
    let title = title.replace(['"', '\''], "");
    let message = message.replace(['"', '\''], "");
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(DETACH_SCRIPT)
        .arg("sh")
        .args(dialog_argv(&title, &message))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

#[cfg(target_os = "macos")]
fn dialog_argv(title: &str, message: &str) -> Vec<String> {
    vec![
        "/usr/bin/osascript".to_owned(),
        "-e".to_owned(),
        format!(
            "display dialog \"{message}\" with title \"{title}\" buttons {{\"OK\"}} with icon stop"
        ),
    ]
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn dialog_argv(title: &str, message: &str) -> Vec<String> {
    vec![
        "notify-send".to_owned(),
        "--urgency=critical".to_owned(),
        title.to_owned(),
        message.to_owned(),
    ]
}
