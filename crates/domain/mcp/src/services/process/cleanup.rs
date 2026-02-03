use anyhow::Result;
use std::process::Command;

fn process_exists(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{}", pid)).exists()
}

pub fn terminate_gracefully(pid: u32) -> Result<()> {
    if !process_exists(pid) {
        tracing::debug!(pid = pid, "Process already terminated, skipping signal");
        return Ok(());
    }

    tracing::debug!(pid = pid, "Sending SIGTERM");

    let output = Command::new("kill")
        .args(["-15", &pid.to_string()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(pid = pid, error = %stderr, "Failed to send SIGTERM");
    }

    Ok(())
}

pub fn force_kill(pid: u32) -> Result<()> {
    if !process_exists(pid) {
        tracing::debug!(pid = pid, "Process already terminated, skipping kill");
        return Ok(());
    }

    tracing::debug!(pid = pid, "Force killing process");

    let output = Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(pid = pid, error = %stderr, "Failed to force kill");
    }

    Ok(())
}

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
