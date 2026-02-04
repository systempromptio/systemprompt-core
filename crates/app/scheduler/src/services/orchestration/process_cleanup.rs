use anyhow::{bail, Result};
use std::process::Command;

const PROTECTED_PORTS: &[u16] = &[5432, 6432];
const PROTECTED_PROCESSES: &[&str] = &["postgres", "pgbouncer", "psql"];

#[derive(Debug, Clone, Copy)]
pub struct ProcessCleanup;

impl ProcessCleanup {
    #[cfg(unix)]
    pub fn check_port(port: u16) -> Option<u32> {
        if PROTECTED_PORTS.contains(&port) {
            return None;
        }

        let output = Command::new("lsof")
            .args(["-ti", &format!(":{}", port)])
            .output()
            .ok()?;

        if output.stdout.is_empty() {
            None
        } else {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .and_then(|pid| pid.trim().parse::<u32>().ok())
        }
    }

    #[cfg(windows)]
    pub fn check_port(port: u16) -> Option<u32> {
        if PROTECTED_PORTS.contains(&port) {
            return None;
        }

        let output = Command::new("netstat")
            .args(["-ano", "-p", "TCP"])
            .output()
            .ok()?;

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

    pub fn kill_port(port: u16) -> Vec<u32> {
        if PROTECTED_PORTS.contains(&port) {
            return vec![];
        }

        let mut killed = vec![];

        if let Some(pid) = Self::check_port(port) {
            if Self::kill_process(pid) {
                killed.push(pid);
            }
        }

        killed
    }

    #[cfg(unix)]
    pub fn kill_process(pid: u32) -> bool {
        use nix::sys::signal::{self, Signal};
        use nix::unistd::Pid;
        signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL).is_ok()
    }

    #[cfg(windows)]
    pub fn kill_process(pid: u32) -> bool {
        Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output()
            .is_ok_and(|output| output.status.success())
    }

    #[cfg(unix)]
    pub async fn terminate_gracefully(pid: u32, grace_period_ms: u64) -> bool {
        use nix::sys::signal::{self, Signal};
        use nix::unistd::Pid;

        if signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM).is_err() {
            return false;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(grace_period_ms)).await;

        if Self::process_exists(pid) {
            Self::kill_process(pid)
        } else {
            true
        }
    }

    #[cfg(windows)]
    pub async fn terminate_gracefully(pid: u32, grace_period_ms: u64) -> bool {
        if Command::new("taskkill")
            .args(["/PID", &pid.to_string()])
            .output()
            .is_err()
        {
            return false;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(grace_period_ms)).await;

        if Self::process_exists(pid) {
            Self::kill_process(pid)
        } else {
            true
        }
    }

    #[cfg(unix)]
    pub fn process_exists(pid: u32) -> bool {
        use nix::sys::signal;
        use nix::unistd::Pid;
        signal::kill(Pid::from_raw(pid as i32), None).is_ok()
    }

    #[cfg(windows)]
    pub fn process_exists(pid: u32) -> bool {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/NH"])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout);
                !stdout.contains("INFO: No tasks") && !stdout.trim().is_empty()
            })
            .unwrap_or(false)
    }

    #[cfg(unix)]
    pub fn kill_by_pattern(pattern: &str) -> usize {
        for protected in PROTECTED_PROCESSES {
            if pattern.contains(protected) {
                return 0;
            }
        }

        usize::from(
            Command::new("pkill")
                .args(["-9", "-f", pattern])
                .output()
                .is_ok_and(|output| output.status.success()),
        )
    }

    #[cfg(windows)]
    pub fn kill_by_pattern(pattern: &str) -> usize {
        for protected in PROTECTED_PROCESSES {
            if pattern.contains(protected) {
                return 0;
            }
        }

        usize::from(
            Command::new("taskkill")
                .args(["/IM", &format!("*{}*", pattern), "/F"])
                .output()
                .is_ok_and(|output| output.status.success()),
        )
    }

    pub async fn wait_for_port_free(port: u16, max_retries: u8, retry_delay_ms: u64) -> Result<()> {
        for attempt in 1..=max_retries {
            if Self::check_port(port).is_none() {
                return Ok(());
            }

            if attempt < max_retries {
                tokio::time::sleep(tokio::time::Duration::from_millis(retry_delay_ms)).await;
            }
        }

        let pid = Self::check_port(port).unwrap_or(0);
        bail!(
            "Port {} still occupied by PID {} after {} attempts",
            port,
            pid,
            max_retries
        )
    }

    #[cfg(unix)]
    pub fn get_process_by_port(port: u16) -> Option<ProcessInfo> {
        let output = Command::new("lsof")
            .args(["-ti", &format!(":{}", port)])
            .output()
            .ok()?;

        let pid: u32 = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()?
            .trim()
            .parse()
            .ok()?;

        let comm_output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "comm="])
            .output()
            .ok()?;

        let name = String::from_utf8_lossy(&comm_output.stdout)
            .trim()
            .to_string();

        Some(ProcessInfo { pid, name, port })
    }

    #[cfg(windows)]
    pub fn get_process_by_port(port: u16) -> Option<ProcessInfo> {
        let pid = Self::check_port(port)?;

        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = stdout.trim().split(',').collect();

        let name = if !parts.is_empty() {
            parts[0].trim_matches('"').to_string()
        } else {
            "unknown".to_string()
        };

        Some(ProcessInfo { pid, name, port })
    }
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub port: u16,
}
