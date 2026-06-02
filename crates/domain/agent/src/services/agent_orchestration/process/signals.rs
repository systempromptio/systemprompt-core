//! Cross-platform process probing and termination primitives.

use crate::services::shared::{AgentServiceError, Result};
#[cfg(windows)]
use std::process::Command;

#[cfg(unix)]
pub fn process_exists(pid: u32) -> bool {
    use nix::sys::signal;
    use nix::unistd::Pid;
    let Some(pid) = systemprompt_models::subprocess::signalable_pid(pid) else {
        return false;
    };
    signal::kill(Pid::from_raw(pid), None).is_ok()
}

#[cfg(windows)]
pub fn process_exists(pid: u32) -> bool {
    match Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output()
    {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            !stdout.contains("INFO: No tasks") && !stdout.trim().is_empty()
        },
        Err(e) => {
            tracing::warn!(
                pid = pid,
                error = %e,
                "failed to run `tasklist` while checking process; assuming dead",
            );
            false
        },
    }
}

#[cfg(unix)]
pub fn terminate_process(pid: u32) -> Result<()> {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    let Some(raw) = systemprompt_models::subprocess::signalable_pid(pid) else {
        return Err(AgentServiceError::Internal(format!(
            "Refusing to signal non-signalable PID {pid}"
        )));
    };

    signal::kill(Pid::from_raw(raw), Signal::SIGTERM).map_err(|e| {
        AgentServiceError::Internal(format!("Failed to send SIGTERM to PID {pid}: {e}"))
    })?;

    Ok(())
}

#[cfg(windows)]
pub fn terminate_process(pid: u32) -> Result<()> {
    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .output()
        .map_err(|e| {
            AgentServiceError::Internal(format!(
                "{}: {e}",
                format!("Failed to run taskkill for PID {pid}")
            ))
        })?;

    if !output.status.success() {
        return Err(AgentServiceError::Internal(format!(
            "taskkill failed for PID {pid}"
        )));
    }
    Ok(())
}

#[cfg(unix)]
pub fn force_kill_process(pid: u32) -> Result<()> {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    let Some(raw) = systemprompt_models::subprocess::signalable_pid(pid) else {
        return Err(AgentServiceError::Internal(format!(
            "Refusing to signal non-signalable PID {pid}"
        )));
    };

    signal::kill(Pid::from_raw(raw), Signal::SIGKILL).map_err(|e| {
        AgentServiceError::Internal(format!("Failed to send SIGKILL to PID {pid}: {e}"))
    })?;

    Ok(())
}

#[cfg(windows)]
pub fn force_kill_process(pid: u32) -> Result<()> {
    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
        .map_err(|e| {
            AgentServiceError::Internal(format!(
                "{}: {e}",
                format!("Failed to force-kill PID {pid}")
            ))
        })?;

    if !output.status.success() {
        return Err(AgentServiceError::Internal(format!(
            "taskkill /F failed for PID {pid}"
        )));
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn verify_process_started(pid: u32) -> bool {
    use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
    use nix::unistd::Pid;

    let Some(raw) = systemprompt_models::subprocess::signalable_pid(pid) else {
        return false;
    };

    match waitpid(Pid::from_raw(raw), Some(WaitPidFlag::WNOHANG)) {
        Ok(WaitStatus::StillAlive) => true,
        Ok(_) => false,
        Err(_) => process_exists(pid),
    }
}

#[cfg(windows)]
pub(super) fn verify_process_started(pid: u32) -> bool {
    process_exists(pid)
}

pub async fn terminate_gracefully(pid: u32, timeout_secs: u64) -> Result<()> {
    if !process_exists(pid) {
        return Ok(());
    }

    terminate_process(pid)?;

    let check_interval = tokio::time::Duration::from_millis(100);
    let max_checks = (timeout_secs * 1000) / 100;

    for _ in 0..max_checks {
        if !process_exists(pid) {
            return Ok(());
        }
        tokio::time::sleep(check_interval).await;
    }

    force_kill_process(pid)?;

    for _ in 0..50 {
        if !process_exists(pid) {
            return Ok(());
        }
        tokio::time::sleep(check_interval).await;
    }

    Err(AgentServiceError::Internal(format!(
        "Failed to kill process {pid} even with SIGKILL"
    )))
}

/// True only if `pid` is alive and still ours.
///
/// Matches this agent's spawn markers (`SYSTEMPROMPT_SUBPROCESS=1` +
/// `AGENT_NAME=<service_name>`) in `/proc/<pid>/environ`.
/// Registry PIDs outlive the processes that minted them and are recycled by the
/// kernel; every signal aimed at a PID believed to be "our agent `<name>`" must
/// gate on this, so `kill`/`force_kill` can never reach an unrelated process.
fn pid_is_agent_child(pid: u32, service_name: &str) -> bool {
    systemprompt_models::subprocess::live_pid_is_subprocess(
        pid,
        systemprompt_models::subprocess::AGENT_NAME_ENV,
        service_name,
    )
}

/// Identity-gated [`terminate_gracefully`].
///
/// Refuses to signal a PID that no longer names this agent (recycled/stale): a
/// PID that fails the marker check is left untouched and reported as
/// terminated, so the caller clears the stale registry row and respawns.
pub async fn terminate_gracefully_verified(
    pid: u32,
    service_name: &str,
    timeout_secs: u64,
) -> Result<()> {
    if !process_exists(pid) {
        return Ok(());
    }

    if !pid_is_agent_child(pid, service_name) {
        tracing::warn!(
            pid,
            service = %service_name,
            "Recorded PID is alive but is not our child (recycled/stale); skipping signal"
        );
        return Ok(());
    }

    terminate_gracefully(pid, timeout_secs).await
}

pub fn kill_process(pid: u32) -> bool {
    terminate_process(pid).is_ok()
}

/// SIGKILL `pid` only if it still names this agent.
///
/// A dead PID, or a recycled one that is no longer ours, counts as already-gone
/// (`true`) and is left unsignalled — so the caller proceeds with registry
/// cleanup without ever killing a stranger.
pub fn kill_process_verified(pid: u32, service_name: &str) -> bool {
    if !process_exists(pid) {
        return true;
    }

    if !pid_is_agent_child(pid, service_name) {
        tracing::warn!(
            pid,
            service = %service_name,
            "Recorded PID is alive but is not our child (recycled/stale); skipping signal"
        );
        return true;
    }

    kill_process(pid)
}
