//! Starting and stopping the server process without a console window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::process::Stdio;

use tokio::process::{Child, Command};
#[cfg(windows)]
use tracing::debug;
use tracing::trace;

use crate::lsp::spec::ServerSpec;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(super) fn server_command(program: &Path, spec: &ServerSpec, root: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .args(&spec.args)
        .envs(&spec.env)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

pub(super) async fn kill(child: &mut Child) {
    #[cfg(windows)]
    if let Some(pid) = child.id() {
        kill_tree(pid).await;
    }
    if let Err(error) = child.kill().await {
        trace!(%error, "language server already gone");
    }
}

// Why: a server started through a .cmd shim runs as a grandchild of cmd.exe;
// killing only the direct child would leave the real server running.
#[cfg(windows)]
async fn kill_tree(pid: u32) {
    let pid_text = pid.to_string();
    let status = Command::new("taskkill")
        .args(["/T", "/F", "/PID", pid_text.as_str()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .await;
    if let Err(error) = status {
        debug!(pid, %error, "taskkill failed");
    }
}

#[cfg(windows)]
pub(super) fn kill_tree_detached(pid: u32) {
    use std::os::windows::process::CommandExt;
    let pid_text = pid.to_string();
    let spawned = std::process::Command::new("taskkill")
        .args(["/T", "/F", "/PID", pid_text.as_str()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    if let Err(error) = spawned {
        debug!(pid, %error, "taskkill failed");
    }
}
