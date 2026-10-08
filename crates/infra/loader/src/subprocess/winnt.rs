//! Windows process primitives behind [`super::control`].
//!
//! Windows exposes no signal or reap syscall to this crate's dependency set,
//! so every primitive here runs `tasklist`/`taskkill` and blocks; the async
//! layer only reaches them through `spawn_blocking`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::Command;

use super::SupervisionError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Delivery {
    Sent,
    NoSuchProcess,
}

pub(super) fn exists(pid: u32) -> bool {
    match Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            !stdout.contains("INFO: No tasks") && !stdout.trim().is_empty()
        },
        Err(e) => {
            tracing::warn!(pid, error = %e, "Failed to run tasklist; treating the process as gone");
            false
        },
    }
}

pub(super) fn has_exited(pid: u32) -> bool {
    !exists(pid)
}

pub(super) const fn is_group_leader(_pid: u32) -> bool {
    true
}

pub(super) const fn process_group(_pid: u32) -> Option<u32> {
    None
}

pub(super) fn send(pid: u32, kill: bool, group: bool) -> Result<Delivery, SupervisionError> {
    if pid == 0 || pid == std::process::id() {
        return Err(SupervisionError::NotSignalable { pid });
    }
    if !exists(pid) {
        return Ok(Delivery::NoSuchProcess);
    }
    let pid_arg = pid.to_string();
    let mut args = vec!["/PID", pid_arg.as_str()];
    if group {
        args.push("/T");
    }
    if kill {
        args.push("/F");
    }
    let output = Command::new("taskkill")
        .args(&args)
        .output()
        .map_err(|source| SupervisionError::Tool {
            tool: "taskkill",
            source,
        })?;
    if output.status.success() {
        Ok(Delivery::Sent)
    } else if exists(pid) {
        Err(SupervisionError::ToolFailed {
            tool: "taskkill",
            status: output.status,
        })
    } else {
        Ok(Delivery::NoSuchProcess)
    }
}
