//! Cross-context privilege escalation for macOS installers.
//!
//! Bridge writes into two root-owned locations on macOS during install
//! (`/Library/Application Support/ClaudeCode/…` and
//! `/Library/Managed Preferences/…`). Which mechanism can prompt the user
//! depends on whether we're on a TTY:
//!
//! - **TTY present** — `sudo /bin/sh -c "…"` prompts on stdin, same as any Unix
//!   installer.
//! - **No TTY** (bridge launched from Finder / launchd via the `.app`) — `sudo`
//!   has nowhere to prompt and hangs. We shell out to `osascript` with `do
//!   shell script … with administrator privileges`, which produces the native
//!   macOS credential dialog (the same one System Preferences uses).
//!
//! Callers hand this module a plain `/bin/sh` script; escaping for
//! `AppleScript` is our problem.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "macos")]

use std::process::Command;

use is_terminal::IsTerminal as _;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ElevationError {
    #[error("user cancelled the administrator authorization prompt")]
    UserCancelled,
    #[error("privileged command failed (exit {status:?}): {stderr}")]
    CommandFailed { status: Option<i32>, stderr: String },
    #[error("failed to spawn privileged command: {0}")]
    Spawn(#[from] std::io::Error),
}

// Why: `prompt` reaches the user only on the GUI path — sudo carries its own
// on a TTY — so it must read as a standalone sentence. `UserCancelled` is a
// decision, not a failure: callers surface it as "declined" rather than error.
// The probe is stdin, not stdout: stdout redirected to a log must not push a
// terminal session onto the osascript dialog path.
pub(crate) fn run_privileged(script: &str, prompt: &str) -> Result<(), ElevationError> {
    if std::io::stdin().is_terminal() {
        sudo_direct(script)
    } else {
        osascript_admin(script, prompt)
    }
}

fn sudo_direct(script: &str) -> Result<(), ElevationError> {
    let output = Command::new("/usr/bin/sudo")
        .arg("/bin/sh")
        .arg("-c")
        .arg(script)
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(ElevationError::CommandFailed {
        status: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn osascript_admin(script: &str, prompt: &str) -> Result<(), ElevationError> {
    use super::elevation_script::applescript_escape;

    let applescript = format!(
        r#"do shell script "{}" with prompt "{}" with administrator privileges"#,
        applescript_escape(script),
        applescript_escape(prompt),
    );
    let output = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(&applescript)
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("(-128)") || stderr.contains("User canceled") {
        return Err(ElevationError::UserCancelled);
    }
    Err(ElevationError::CommandFailed {
        status: output.status.code(),
        stderr: stderr.into_owned(),
    })
}
