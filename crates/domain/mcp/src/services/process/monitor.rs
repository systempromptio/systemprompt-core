use crate::error::McpDomainResult;
use std::process::Command;
use std::time::Duration;

use super::utils;

const HEALTH_CHECK_TIMEOUT_SECS: u64 = 5;

pub async fn is_service_healthy(port: u16) -> McpDomainResult<bool> {
    is_port_responsive(port).await
}

async fn is_port_responsive(port: u16) -> McpDomainResult<bool> {
    use tokio::net::TcpStream;
    use tokio::time::timeout;

    match timeout(
        Duration::from_secs(HEALTH_CHECK_TIMEOUT_SECS),
        TcpStream::connect(format!("127.0.0.1:{port}")),
    )
    .await
    {
        Ok(Ok(_)) => Ok(true),
        _ => Ok(false),
    }
}

pub fn is_process_running(pid: u32) -> bool {
    utils::process_exists(pid)
}

pub fn get_process_info(pid: u32) -> McpDomainResult<Option<ProcessInfo>> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "pid,ppid,cmd"])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "failed to run `ps -p {pid} -o pid,ppid,cmd`: {e}"
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

    let pid: u32 = parts[0].parse().map_err(|_e| {
        crate::error::McpDomainError::Internal(format!("Invalid PID: {}", parts[0]))
    })?;
    let parent_pid: u32 = parts[1].parse().map_err(|_e| {
        crate::error::McpDomainError::Internal(format!("Invalid PPID: {}", parts[1]))
    })?;

    if pid == 0 {
        return Err(crate::error::McpDomainError::Internal(
            "PID cannot be 0".to_owned(),
        ));
    }

    Ok(Some(ProcessInfo {
        pid,
        ppid: parent_pid,
        command: parts[2..].join(" "),
    }))
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub command: String,
}
