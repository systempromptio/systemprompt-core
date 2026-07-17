//! Windows (`#[cfg(windows)]`) backend for [`super::ProcessCleanup`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::Command;

use super::ProcessInfo;

fn is_safe_pattern(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 128
        && p.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

pub(super) fn check_port(port: u16) -> Option<u32> {
    let output = match Command::new("netstat").args(["-ano", "-p", "TCP"]).output() {
        Ok(output) => output,
        Err(e) => {
            tracing::warn!(
                port = port,
                error = %e,
                "failed to run `netstat -ano -p TCP` while checking port; treating as unknown",
            );
            return None;
        },
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let port_pattern = format!(":{} ", port);

    for line in stdout.lines() {
        if line.contains(&port_pattern) {
            if let Some(pid_str) = line.split_whitespace().last() {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    return Some(pid);
                }
            }
        }
    }

    None
}

pub(super) fn kill_process(pid: u32) -> bool {
    match Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
    {
        Ok(output) => output.status.success(),
        Err(e) => {
            tracing::warn!(pid = pid, error = %e, "failed to run `taskkill /PID {pid} /F`");
            false
        },
    }
}

pub(super) async fn terminate_gracefully(pid: u32, grace_period_ms: u64) -> bool {
    if let Err(e) = Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .output()
    {
        tracing::warn!(pid = pid, error = %e, "failed to run `taskkill /PID {pid}`");
        return false;
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(grace_period_ms)).await;

    if process_exists(pid) {
        kill_process(pid)
    } else {
        true
    }
}

pub(super) async fn terminate_group_gracefully(pid: u32, grace_period_ms: u64) -> bool {
    // `taskkill /T` terminates the process and any child processes it started,
    // the closest Windows analogue to POSIX process-group signalling.
    if let Err(e) = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T"])
        .output()
    {
        tracing::warn!(pid = pid, error = %e, "failed to run `taskkill /T /PID {pid}`");
        return terminate_gracefully(pid, grace_period_ms).await;
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(grace_period_ms)).await;

    if process_exists(pid) {
        kill_process(pid)
    } else {
        true
    }
}

pub(super) fn process_group(_pid: u32) -> Option<u32> {
    None
}

pub(super) fn process_exists(pid: u32) -> bool {
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

pub(super) fn kill_by_pattern(pattern: &str) -> usize {
    if !is_safe_pattern(pattern) {
        tracing::warn!(pattern = %pattern, "rejecting kill_by_pattern: pattern contains unsafe characters");
        return 0;
    }
    match Command::new("taskkill")
        .args(["/IM", &format!("*{}*", pattern), "/F"])
        .output()
    {
        Ok(output) => usize::from(output.status.success()),
        Err(e) => {
            tracing::warn!(
                pattern = %pattern,
                error = %e,
                "failed to run `taskkill /IM *{pattern}* /F`",
            );
            0
        },
    }
}

pub(super) fn get_process_by_port(port: u16) -> Option<ProcessInfo> {
    let pid = check_port(port)?;

    let output = match Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            tracing::warn!(
                pid = pid,
                error = %e,
                "failed to run `tasklist` while inspecting process",
            );
            return None;
        },
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = stdout.trim().split(',').collect();

    let name = if !parts.is_empty() {
        parts[0].trim_matches('"').to_string()
    } else {
        "unknown".to_string()
    };

    Some(ProcessInfo { pid, name, port })
}
