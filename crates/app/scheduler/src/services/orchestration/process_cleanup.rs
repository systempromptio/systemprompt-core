use anyhow::{Result, bail};
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

        let output = match Command::new("lsof")
            .args(["-ti", &format!(":{}", port)])
            .output()
        {
            Ok(output) => output,
            Err(e) => {
                tracing::warn!(
                    port = port,
                    error = %e,
                    "failed to run `lsof -ti :{port}` while checking port; treating as unknown",
                );
                return None;
            },
        };

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

        let output = match Command::new("netstat")
            .args(["-ano", "-p", "TCP"])
            .output()
        {
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
        if let Err(e) = Command::new("taskkill")
            .args(["/PID", &pid.to_string()])
            .output()
        {
            tracing::warn!(pid = pid, error = %e, "failed to run `taskkill /PID {pid}`");
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
    pub fn kill_by_pattern(pattern: &str) -> usize {
        for protected in PROTECTED_PROCESSES {
            if pattern.contains(protected) {
                return 0;
            }
        }

        match Command::new("pkill").args(["-9", "-f", pattern]).output() {
            Ok(output) => usize::from(output.status.success()),
            Err(e) => {
                tracing::warn!(
                    pattern = %pattern,
                    error = %e,
                    "failed to run `pkill -9 -f {pattern}`",
                );
                0
            },
        }
    }

    #[cfg(windows)]
    pub fn kill_by_pattern(pattern: &str) -> usize {
        for protected in PROTECTED_PROCESSES {
            if pattern.contains(protected) {
                return 0;
            }
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

    pub async fn wait_for_port_free(port: u16, max_retries: u8, retry_delay_ms: u64) -> Result<()> {
        for attempt in 1..=max_retries {
            if Self::check_port(port).is_none() {
                return Ok(());
            }

            if attempt < max_retries {
                tokio::time::sleep(tokio::time::Duration::from_millis(retry_delay_ms)).await;
            }
        }

        match Self::check_port(port) {
            Some(pid) => bail!(
                "Port {} still occupied by PID {} after {} attempts",
                port,
                pid,
                max_retries
            ),
            None => bail!(
                "Port {} still occupied by unknown process after {} attempts",
                port,
                max_retries
            ),
        }
    }

    #[cfg(unix)]
    pub fn get_process_by_port(port: u16) -> Option<ProcessInfo> {
        let output = match Command::new("lsof")
            .args(["-ti", &format!(":{}", port)])
            .output()
        {
            Ok(output) => output,
            Err(e) => {
                tracing::warn!(
                    port = port,
                    error = %e,
                    "failed to run `lsof -ti :{port}` while inspecting port; treating as unknown",
                );
                return None;
            },
        };

        let pid: u32 = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()?
            .trim()
            .parse()
            .ok()?;

        let comm_output = match Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "comm="])
            .output()
        {
            Ok(output) => output,
            Err(e) => {
                tracing::warn!(
                    pid = pid,
                    error = %e,
                    "failed to run `ps -p {pid} -o comm=` while inspecting process",
                );
                return None;
            },
        };

        let name = String::from_utf8_lossy(&comm_output.stdout)
            .trim()
            .to_string();

        Some(ProcessInfo { pid, name, port })
    }

    #[cfg(windows)]
    pub fn get_process_by_port(port: u16) -> Option<ProcessInfo> {
        let pid = Self::check_port(port)?;

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
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub port: u16,
}
