//! Cross-platform process / port probing helpers.

use crate::services::shared::{AgentServiceError, Result};
use std::process::Command;
use systemprompt_models::CliPaths;

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub command: String,
}

#[cfg(unix)]
pub fn find_process_using_port(port: u16) -> Result<Option<u32>> {
    let output = Command::new("lsof")
        .arg("-ti")
        .arg(format!(":{port}"))
        .output()
        .map_err(|e| {
            AgentServiceError::Internal(format!(
                "failed to run `lsof -ti :{port}` for port {port}: {e}"
            ))
        })?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pid_str = stdout.trim();

    if pid_str.is_empty() {
        return Ok(None);
    }

    let pid = pid_str.parse::<u32>().map_err(|e| {
        AgentServiceError::Internal(format!("Failed to parse PID from lsof output: {e}"))
    })?;

    Ok(Some(pid))
}

#[cfg(windows)]
pub fn find_process_using_port(port: u16) -> Result<Option<u32>> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .map_err(|e| {
            AgentServiceError::Internal(format!(
                "{}: {e}",
                format!("failed to run `netstat -ano -p TCP` for port {port}")
            ))
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let port_pattern = format!(":{port} ");
    let port_pattern_tab = format!(":{port}\t");

    for line in stdout.lines() {
        if line.contains(&port_pattern) || line.contains(&port_pattern_tab) {
            if let Some(pid_str) = line.split_whitespace().last() {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    return Ok(Some(pid));
                }
            }
        }
    }

    Ok(None)
}

#[cfg(unix)]
pub fn get_process_info(pid: u32) -> Result<Option<ProcessInfo>> {
    let output = Command::new("ps")
        .arg("-p")
        .arg(pid.to_string())
        .arg("-o")
        .arg("pid,comm,args")
        .output()
        .map_err(|e| {
            AgentServiceError::Internal(format!(
                "failed to run `ps -p {pid} -o pid,comm,args`: {e}"
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

    let line = lines[1].trim();
    if line.is_empty() {
        return Ok(None);
    }

    let parts: Vec<&str> = line.splitn(3, char::is_whitespace).collect();
    if parts.len() < 3 {
        return Ok(None);
    }

    let command_line = parts[2].trim();

    Ok(Some(ProcessInfo {
        pid,
        command: command_line.to_string(),
    }))
}

#[cfg(windows)]
pub fn get_process_info(pid: u32) -> Result<Option<ProcessInfo>> {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
        .output()
        .map_err(|e| {
            AgentServiceError::Internal(format!(
                "{}: {e}",
                format!("failed to run `tasklist /FI PID eq {pid}`")
            ))
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.trim();

    if line.is_empty() || line.contains("INFO: No tasks") {
        return Ok(None);
    }

    let parts: Vec<&str> = line.split(',').collect();
    if parts.is_empty() {
        return Ok(None);
    }

    let command = parts[0].trim_matches('"').to_string();

    Ok(Some(ProcessInfo { pid, command }))
}

pub fn is_agent_process(pid: u32) -> std::result::Result<bool, String> {
    match get_process_info(pid) {
        Ok(Some(info)) => {
            let is_agent = info.command.contains("systemprompt")
                && (info.command.contains(CliPaths::agent_run_cmd_pattern())
                    || info.command.contains("agent-worker"));
            Ok(is_agent)
        },
        Ok(None) => Err(format!("No process info found for PID {}", pid)),
        Err(e) => Err(format!("Failed to get process info for PID {}: {}", pid, e)),
    }
}
