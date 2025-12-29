use anyhow::{bail, Result};
use std::path::Path;
use std::process::Command;

const PROTECTED_PORTS: &[u16] = &[5432, 6432];
const PROTECTED_PROCESSES: &[&str] = &["postgres", "pgbouncer", "psql"];

#[derive(Debug, Clone, Copy)]
pub struct ProcessCleanup;

impl ProcessCleanup {
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

    pub fn kill_port(port: u16) -> Vec<u32> {
        if PROTECTED_PORTS.contains(&port) {
            return vec![];
        }

        let mut killed = vec![];

        if let Ok(output) = Command::new("lsof")
            .args(["-ti", &format!(":{}", port)])
            .output()
        {
            let pids = String::from_utf8_lossy(&output.stdout);
            for pid_str in pids.lines() {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    if Self::kill_process(pid) {
                        killed.push(pid);
                    }
                }
            }
        }

        killed
    }

    pub fn kill_process(pid: u32) -> bool {
        Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output()
            .is_ok_and(|output| output.status.success())
    }

    pub async fn terminate_gracefully(pid: u32, grace_period_ms: u64) -> bool {
        if Command::new("kill")
            .args(["-15", &pid.to_string()])
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

    pub fn process_exists(pid: u32) -> bool {
        Path::new(&format!("/proc/{}", pid)).exists()
    }

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
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub port: u16,
}
