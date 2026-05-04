//! Cross-platform process probing and termination primitives.

use anyhow::{Context, Result};
#[cfg(windows)]
use std::process::Command;

#[cfg(unix)]
pub fn process_exists(pid: u32) -> bool {
    use nix::sys::signal;
    use nix::unistd::Pid;
    signal::kill(Pid::from_raw(pid as i32), None).is_ok()
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

    signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM)
        .with_context(|| format!("Failed to send SIGTERM to PID {pid}"))?;

    Ok(())
}

#[cfg(windows)]
pub fn terminate_process(pid: u32) -> Result<()> {
    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .output()
        .with_context(|| format!("Failed to run taskkill for PID {pid}"))?;

    if !output.status.success() {
        anyhow::bail!("taskkill failed for PID {pid}");
    }
    Ok(())
}

#[cfg(unix)]
pub fn force_kill_process(pid: u32) -> Result<()> {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL)
        .with_context(|| format!("Failed to send SIGKILL to PID {pid}"))?;

    Ok(())
}

#[cfg(windows)]
pub fn force_kill_process(pid: u32) -> Result<()> {
    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
        .with_context(|| format!("Failed to force-kill PID {pid}"))?;

    if !output.status.success() {
        anyhow::bail!("taskkill /F failed for PID {pid}");
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn verify_process_started(pid: u32) -> bool {
    use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
    use nix::unistd::Pid;

    match waitpid(Pid::from_raw(pid as i32), Some(WaitPidFlag::WNOHANG)) {
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

    Err(anyhow::anyhow!(
        "Failed to kill process {} even with SIGKILL",
        pid
    ))
}

pub fn kill_process(pid: u32) -> bool {
    terminate_process(pid).is_ok()
}
