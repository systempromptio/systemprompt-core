//! Process termination and port reclamation for MCP servers.
//!
//! Cross-platform helpers to gracefully terminate ([`terminate_gracefully`])
//! then force-kill ([`force_kill`]) a process by PID, and to discover and clear
//! every process holding a given port. All operations are idempotent against an
//! already-dead PID.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use std::process::Command;

use super::utils::process_exists;

#[cfg(unix)]
pub fn terminate_gracefully(pid: u32) -> McpDomainResult<()> {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    // Why: Never signal the caller: a misresolved port/name lookup must not let
    // server cleanup terminate this process.
    if pid == std::process::id() {
        return Ok(());
    }

    if !process_exists(pid) {
        tracing::debug!(pid = pid, "Process already terminated, skipping signal");
        return Ok(());
    }

    tracing::debug!(pid = pid, "Sending SIGTERM");

    signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM).map_err(|e| {
        crate::error::McpDomainError::Internal(format!("Failed to send SIGTERM to PID {pid}: {e}"))
    })?;

    Ok(())
}

#[cfg(windows)]
pub fn terminate_gracefully(pid: u32) -> McpDomainResult<()> {
    if !process_exists(pid) {
        tracing::debug!(pid = pid, "Process already terminated, skipping signal");
        return Ok(());
    }

    tracing::debug!(pid = pid, "Sending termination signal");

    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "{}: {e}",
                format!("failed to run `taskkill /PID {pid}`")
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(pid = pid, error = %stderr, "Failed to send termination signal");
    }

    Ok(())
}

#[cfg(unix)]
pub fn force_kill(pid: u32) -> McpDomainResult<()> {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    // Why: Never signal the caller: a misresolved port/name lookup must not let
    // server cleanup terminate this process.
    if pid == std::process::id() {
        return Ok(());
    }

    if !process_exists(pid) {
        tracing::debug!(pid = pid, "Process already terminated, skipping kill");
        return Ok(());
    }

    tracing::debug!(pid = pid, "Force killing process");

    signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL).map_err(|e| {
        crate::error::McpDomainError::Internal(format!("Failed to force kill PID {pid}: {e}"))
    })?;

    Ok(())
}

#[cfg(windows)]
pub fn force_kill(pid: u32) -> McpDomainResult<()> {
    if !process_exists(pid) {
        tracing::debug!(pid = pid, "Process already terminated, skipping kill");
        return Ok(());
    }

    tracing::debug!(pid = pid, "Force killing process");

    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "{}: {e}",
                format!("failed to run `taskkill /PID {pid} /F`")
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(pid = pid, error = %stderr, "Failed to force kill");
    }

    Ok(())
}

/// Graceful-then-forced termination gated on identity.
///
/// Signals `pid` only once its `/proc/<pid>/environ` still carries this
/// server's spawn markers (`SYSTEMPROMPT_SUBPROCESS=1` +
/// `MCP_SERVICE_ID=<service_name>`). Every site that signals a PID it believes
/// is "our MCP server `<name>`" must route through here. Registry and
/// port-discovered PIDs outlive the processes that minted them and are recycled
/// by the kernel; a recycled PID that fails the marker check is left untouched,
/// so `kill`/`kill(-pid)` can never reach an unrelated process (or, via the
/// group, an unrelated session leader). Use the bare [`terminate_gracefully`] /
/// [`force_kill`] only for port-reclamation, where there is no service identity
/// to verify against.
pub async fn terminate_gracefully_verified(pid: u32, service_name: &str) -> McpDomainResult<()> {
    if !process_exists(pid) {
        return Ok(());
    }

    if !systemprompt_models::subprocess::live_pid_is_subprocess(
        pid,
        systemprompt_models::subprocess::MCP_SERVICE_ID_ENV,
        service_name,
    ) {
        tracing::warn!(
            pid,
            service = %service_name,
            "Recorded PID is alive but is not our child (recycled/stale); skipping signal"
        );
        return Ok(());
    }

    terminate_gracefully(pid)?;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    force_kill(pid)
}

#[cfg(unix)]
pub async fn cleanup_port_processes(port: u16) -> McpDomainResult<Vec<u32>> {
    tracing::debug!(port = port, "Cleaning up processes on port");

    let output = Command::new("lsof")
        .args(["-ti", &format!(":{port}")])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "failed to run `lsof -ti :{port}` for port {port}: {e}"
            ))
        })?;

    if output.stdout.is_empty() {
        return Ok(vec![]);
    }

    let pids_string = String::from_utf8_lossy(&output.stdout);
    let mut killed_pids = Vec::new();

    for pid_str in pids_string.lines() {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            tracing::debug!(port = port, pid = pid, "Stopping process blocking port");

            terminate_gracefully(pid)?;

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            force_kill(pid)?;

            killed_pids.push(pid);
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    Ok(killed_pids)
}

#[cfg(windows)]
pub async fn cleanup_port_processes(port: u16) -> McpDomainResult<Vec<u32>> {
    tracing::debug!(port = port, "Cleaning up processes on port");

    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "{}: {e}",
                format!("failed to run `netstat -ano -p TCP` for port {port}")
            ))
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let port_pattern = format!(":{port} ");
    let mut killed_pids = Vec::new();

    for line in stdout.lines() {
        if line.contains(&port_pattern) {
            if let Some(pid_str) = line.split_whitespace().last() {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    tracing::debug!(port = port, pid = pid, "Stopping process blocking port");

                    terminate_gracefully(pid)?;

                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                    force_kill(pid)?;

                    killed_pids.push(pid);
                }
            }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    Ok(killed_pids)
}
