//! Cross-platform PID and port lookup utilities for MCP service supervision.
//!
//! On Linux, prefers `/proc` parsing (no subprocess); falls back to `lsof`.
//! On other Unix targets, uses `lsof`. On Windows, parses `netstat`/`tasklist`.

use crate::error::McpDomainResult;
use std::process::Command;

#[cfg(target_os = "linux")]
#[path = "pid/linux_proc.rs"]
mod linux_proc;

#[cfg(target_os = "linux")]
pub fn find_pid_by_port(port: u16) -> McpDomainResult<Option<u32>> {
    if let Some(pid) = linux_proc::find_pid_by_port_proc(port) {
        return Ok(Some(pid));
    }

    find_pid_by_port_lsof(port)
}

#[cfg(all(unix, not(target_os = "linux")))]
pub fn find_pid_by_port(port: u16) -> McpDomainResult<Option<u32>> {
    find_pid_by_port_lsof(port)
}

#[cfg(unix)]
fn find_pid_by_port_lsof(port: u16) -> McpDomainResult<Option<u32>> {
    let output = Command::new("lsof")
        .args(["-ti", &format!(":{port}")])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "failed to run `lsof -ti :{port}` for port {port}: {e}"
            ))
        })?;

    if output.stdout.is_empty() {
        return Ok(None);
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .and_then(|line| line.trim().parse::<u32>().ok()))
}

#[cfg(windows)]
pub fn find_pid_by_port(port: u16) -> McpDomainResult<Option<u32>> {
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
pub fn find_pids_by_name(process_name: &str) -> McpDomainResult<Vec<u32>> {
    let output = Command::new("pgrep")
        .args(["-f", process_name])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "failed to run `pgrep -f {process_name}`: {e}"
            ))
        })?;

    if output.stdout.is_empty() {
        return Ok(vec![]);
    }

    let pids = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect();

    Ok(pids)
}

#[cfg(windows)]
pub fn find_pids_by_name(process_name: &str) -> McpDomainResult<Vec<u32>> {
    let output = Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "{}: {e}",
                format!("failed to run `tasklist` searching for {process_name}")
            ))
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut pids = Vec::new();

    for line in stdout.lines() {
        if line.to_lowercase().contains(&process_name.to_lowercase()) {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 2 {
                if let Ok(pid) = parts[1].trim_matches('"').parse::<u32>() {
                    pids.push(pid);
                }
            }
        }
    }

    Ok(pids)
}

#[cfg(target_os = "linux")]
pub fn get_port_by_pid(pid: u32) -> McpDomainResult<Option<u16>> {
    if let Some(port) = linux_proc::get_port_by_pid_proc(pid) {
        return Ok(Some(port));
    }

    get_port_by_pid_lsof(pid)
}

#[cfg(all(unix, not(target_os = "linux")))]
pub fn get_port_by_pid(pid: u32) -> McpDomainResult<Option<u16>> {
    get_port_by_pid_lsof(pid)
}

#[cfg(unix)]
fn get_port_by_pid_lsof(pid: u32) -> McpDomainResult<Option<u16>> {
    let output = Command::new("lsof")
        .args(["-p", &pid.to_string(), "-P", "-n"])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "failed to run `lsof -p {pid} -P -n` for pid {pid}: {e}"
            ))
        })?;

    if !output.status.success() {
        return Ok(None);
    }

    let port = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| line.contains("LISTEN"))
        .and_then(|line| {
            line.split_whitespace()
                .find(|part| part.contains(':'))
                .and_then(|part| part.split(':').next_back())
                .and_then(|port_part| port_part.parse::<u16>().ok())
        });

    Ok(port)
}

#[cfg(windows)]
pub fn get_port_by_pid(pid: u32) -> McpDomainResult<Option<u16>> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .map_err(|e| {
            crate::error::McpDomainError::Internal(format!(
                "{}: {e}",
                format!("failed to run `netstat -ano -p TCP` for pid {pid}")
            ))
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pid_str = pid.to_string();

    for line in stdout.lines() {
        if line.contains("LISTENING") && line.ends_with(&pid_str) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Some(port_str) = parts[1].split(':').last() {
                    if let Ok(port) = port_str.parse::<u16>() {
                        return Ok(Some(port));
                    }
                }
            }
        }
    }

    Ok(None)
}

#[cfg(unix)]
pub fn get_process_name_by_pid(pid: u32) -> Option<String> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let name = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if name.is_empty() { None } else { Some(name) }
}

#[cfg(windows)]
pub fn get_process_name_by_pid(pid: u32) -> Option<String> {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.trim();

    if line.is_empty() || line.contains("INFO: No tasks") {
        return None;
    }

    let parts: Vec<&str> = line.split(',').collect();
    if parts.is_empty() {
        return None;
    }

    Some(parts[0].trim_matches('"').to_string())
}

pub fn find_process_on_port_with_name(
    port: u16,
    expected_name: &str,
) -> McpDomainResult<Option<u32>> {
    let Some(pid) = find_pid_by_port(port)? else {
        return Ok(None);
    };

    let Some(actual_name) = get_process_name_by_pid(pid) else {
        return Ok(None);
    };

    if actual_name == expected_name {
        Ok(Some(pid))
    } else {
        Ok(None)
    }
}
