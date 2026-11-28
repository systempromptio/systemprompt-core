//! Windows: paths carry an upper-case drive letter, a command runs only under
//! a `PATHEXT` name, and a server is stopped through `taskkill` over its
//! process tree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::{Child, Command};
use tracing::debug;

use super::Platform;
use crate::lsp::uri::{self, PathStyle};

pub(super) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(super) struct Windows;

impl Platform for Windows {
    const PATH_STYLE: PathStyle = PathStyle::Windows;

    fn settle(path: PathBuf) -> PathBuf {
        let lifted = {
            let text = path.to_string_lossy();
            uri::has_drive(&text).then(|| uri::upper_drive(&text))
        };
        lifted.map_or(path, PathBuf::from)
    }

    // Why: an npm-installed server ships an extensionless shell script beside its
    // .cmd shim, and CreateProcess cannot run the script; only names carrying a
    // PATHEXT extension are runnable.
    fn runnable(candidate: &Path, extensions: &[String]) -> Option<PathBuf> {
        let known = candidate
            .extension()
            .map(|ext| format!(".{}", ext.to_string_lossy().to_ascii_lowercase()))
            .is_some_and(|ext| extensions.contains(&ext));
        if known && candidate.is_file() {
            return Some(candidate.to_path_buf());
        }
        extensions.iter().find_map(|ext| {
            let mut name = candidate.as_os_str().to_owned();
            name.push(ext);
            let path = PathBuf::from(name);
            path.is_file().then_some(path)
        })
    }

    // Why: a server started through a .cmd shim runs as a grandchild of cmd.exe;
    // killing only the direct child would leave the real server running.
    async fn end_tree(child: &Child) {
        let Some(pid) = child.id() else { return };
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

    fn abandon_tree(child: &mut Child) {
        use std::os::windows::process::CommandExt as _;
        if !matches!(child.try_wait(), Ok(None)) {
            return;
        }
        let Some(pid) = child.id() else { return };
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
}
