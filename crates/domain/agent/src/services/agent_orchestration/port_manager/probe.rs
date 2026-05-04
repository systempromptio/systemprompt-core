//! Cross-platform process / port probing helpers.

use anyhow::{Context, Result};
use std::process::Command;
use systemprompt_models::CliPaths;

/// Description of a running process used for port-collision diagnostics.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Process id.
    pub pid: u32,
    /// Full command line of the process.
    pub command: String,
}

/// Resolve the PID of the process bound to `port`, if any.
///
/// # Errors
/// Returns the underlying `lsof`/`netstat` failure.
#[cfg(unix)]
pub fn find_process_using_port(port: u16) -> Result<Option<u32>> {
    let output = Command::new("lsof")
        .arg("-ti")
        .arg(format!(":{port}"))
        .output()
        .with_context(|| format!("failed to run `lsof -ti :{port}` for port {port}"))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pid_str = stdout.trim();

    if pid_str.is_empty() {
        return Ok(None);
    }

    let pid = pid_str
        .parse::<u32>()
        .context("Failed to parse PID from lsof output")?;

    Ok(Some(pid))
}

/// Resolve the PID of the process bound to `port`, if any.
///
/// # Errors
/// Returns the underlying `lsof`/`netstat` failure.
#[cfg(windows)]
pub fn find_process_using_port(port: u16) -> Result<Option<u32>> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .with_context(|| format!("failed to run `netstat -ano -p TCP` for port {port}"))?;

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

/// Look up [`ProcessInfo`] for `pid`.
///
/// # Errors
/// Returns the underlying `ps`/`tasklist` failure.
#[cfg(unix)]
pub fn get_process_info(pid: u32) -> Result<Option<ProcessInfo>> {
    let output = Command::new("ps")
        .arg("-p")
        .arg(pid.to_string())
        .arg("-o")
        .arg("pid,comm,args")
        .output()
        .with_context(|| format!("failed to run `ps -p {pid} -o pid,comm,args`"))?;

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

/// Look up [`ProcessInfo`] for `pid`.
///
/// # Errors
/// Returns the underlying `ps`/`tasklist` failure.
#[cfg(windows)]
pub fn get_process_info(pid: u32) -> Result<Option<ProcessInfo>> {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
        .output()
        .with_context(|| format!("failed to run `tasklist /FI PID eq {pid}`"))?;

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

/// Determine whether `pid` is one of the platform's agent worker processes.
///
/// # Errors
/// Returns a textual message if the process info cannot be obtained.
pub fn is_agent_process(pid: u32) -> Result<bool, String> {
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
