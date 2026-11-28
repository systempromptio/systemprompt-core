//! Windows: a command is resolved to its full path before it is started, and
//! a local server is stopped through `taskkill` over its process tree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::os::windows::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use super::Platform;

pub(super) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(super) struct Windows;

impl Platform for Windows {
    // Why: CreateProcess neither searches PATHEXT nor finds .cmd/.bat shims such as
    // npx, so the full path is resolved first.
    fn resolve_program(
        command: &str,
        env: &BTreeMap<String, String>,
        cwd: Option<&Path>,
    ) -> PathBuf {
        let search_path = env
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("PATH"))
            .map(|(_, value)| value.clone());
        let found = search_path.map_or_else(
            || which::which(command),
            |paths| which::which_in(command, Some(paths), cwd.unwrap_or_else(|| Path::new("."))),
        );
        match found {
            Ok(path) => path,
            Err(error) => {
                tracing::debug!(command, error = %error, "could not resolve the command; starting it as given");
                PathBuf::from(command)
            },
        }
    }

    // Why: killing the direct child leaves the node or python process a shim
    // started still running. taskkill runs on its own; nothing waits for it, so
    // a drop never blocks.
    fn end_tree(pid: u32) {
        let pid = pid.to_string();
        let started = std::process::Command::new("taskkill")
            .args(["/PID", pid.as_str(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
        match started {
            Ok(taskkill) => drop(taskkill),
            Err(error) => tracing::debug!(error = %error, "could not start taskkill"),
        }
    }
}
