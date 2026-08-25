//! The one place this crate shells out to `ps`, normalising the BSD/GNU
//! differences that broke lookups on macOS.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::Command;

use crate::error::McpDomainResult;

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub command: String,
}

#[cfg(unix)]
pub(super) fn command_name(pid: u32) -> Option<String> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    // Why: `comm` is a bare command name under GNU `ps` but the executable's
    // full path under BSD `ps` on macOS. Callers compare the result against a
    // configured server name, so an unnormalised path never matched there and
    // every by-name port lookup silently returned `None`.
    let name = std::path::Path::new(raw.trim())
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_else(|| raw.trim())
        .to_owned();

    if name.is_empty() { None } else { Some(name) }
}

pub(super) fn process_info(pid: u32) -> McpDomainResult<Option<ProcessInfo>> {
    // Why: `command` is the one spelling both `ps` implementations accept —
    // BSD `ps` on macOS rejects the GNU-only `cmd` keyword outright, which made
    // every lookup here return `None` on that platform.
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "pid,ppid,command"])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "failed to run `ps -p {pid} -o pid,ppid,command`: {e}"
            ))
        })?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    if lines.len() < 2 {
        return Ok(None);
    }

    let parts: Vec<&str> = lines[1].split_whitespace().collect();
    if parts.len() < 3 {
        return Ok(None);
    }

    let reported_pid: u32 = parts[0].parse().map_err(|_e| {
        crate::error::McpDomainError::Internal(format!("Invalid PID: {}", parts[0]))
    })?;
    let parent_pid: u32 = parts[1].parse().map_err(|_e| {
        crate::error::McpDomainError::Internal(format!("Invalid PPID: {}", parts[1]))
    })?;

    if reported_pid == 0 {
        return Err(crate::error::McpDomainError::Internal(
            "PID cannot be 0".to_owned(),
        ));
    }

    Ok(Some(ProcessInfo {
        pid: reported_pid,
        ppid: parent_pid,
        command: parts[2..].join(" "),
    }))
}
