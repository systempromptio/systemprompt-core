use anyhow::Result;
use std::process::Command;

use super::utils::process_exists;

#[cfg(unix)]
#[allow(clippy::unnecessary_wraps)]
pub fn terminate_gracefully(pid: u32) -> Result<()> {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    if !process_exists(pid) {
        tracing::debug!(pid = pid, "Process already terminated, skipping signal");
        return Ok(());
    }

    tracing::debug!(pid = pid, "Sending SIGTERM");

    if let Err(e) = signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
        tracing::warn!(pid = pid, error = %e, "Failed to send SIGTERM");
    }

    Ok(())
}

#[cfg(windows)]
pub fn terminate_gracefully(pid: u32) -> Result<()> {
    if !process_exists(pid) {
        tracing::debug!(pid = pid, "Process already terminated, skipping signal");
        return Ok(());
    }

    tracing::debug!(pid = pid, "Sending termination signal");

    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(pid = pid, error = %stderr, "Failed to send termination signal");
    }

    Ok(())
}

#[cfg(unix)]
#[allow(clippy::unnecessary_wraps)]
pub fn force_kill(pid: u32) -> Result<()> {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    if !process_exists(pid) {
        tracing::debug!(pid = pid, "Process already terminated, skipping kill");
        return Ok(());
    }

    tracing::debug!(pid = pid, "Force killing process");

    if let Err(e) = signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL) {
        tracing::warn!(pid = pid, error = %e, "Failed to force kill");
    }

    Ok(())
}

#[cfg(windows)]
pub fn force_kill(pid: u32) -> Result<()> {
    if !process_exists(pid) {
        tracing::debug!(pid = pid, "Process already terminated, skipping kill");
        return Ok(());
    }

    tracing::debug!(pid = pid, "Force killing process");

    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(pid = pid, error = %stderr, "Failed to force kill");
    }

    Ok(())
}

#[cfg(unix)]
pub async fn cleanup_port_processes(port: u16) -> Result<Vec<u32>> {
    tracing::debug!(port = port, "Cleaning up processes on port");

    let output = Command::new("lsof")
        .args(["-ti", &format!(":{port}")])
        .output()?;

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
pub async fn cleanup_port_processes(port: u16) -> Result<Vec<u32>> {
    tracing::debug!(port = port, "Cleaning up processes on port");

    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()?;

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
