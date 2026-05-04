//! Cross-platform process and port cleanup primitives.
//!
//! [`ProcessCleanup`] exposes a uniform API; the platform-specific
//! implementations live in `posix` (Unix) and `winnt` (Windows) and are
//! gated by `#[cfg(unix)]` / `#[cfg(windows)]`.

use crate::error::{SchedulerError, SchedulerResult};

#[cfg(unix)]
mod posix;
#[cfg(windows)]
mod winnt;

const PROTECTED_PORTS: &[u16] = &[5432, 6432];
const PROTECTED_PROCESSES: &[&str] = &["postgres", "pgbouncer", "psql"];

#[derive(Debug, Clone, Copy)]
pub struct ProcessCleanup;

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub port: u16,
}

impl ProcessCleanup {
    #[cfg(unix)]
    pub fn check_port(port: u16) -> Option<u32> {
        if PROTECTED_PORTS.contains(&port) {
            return None;
        }
        posix::check_port(port)
    }

    #[cfg(windows)]
    pub fn check_port(port: u16) -> Option<u32> {
        if PROTECTED_PORTS.contains(&port) {
            return None;
        }
        winnt::check_port(port)
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
        posix::kill_process(pid)
    }

    #[cfg(windows)]
    pub fn kill_process(pid: u32) -> bool {
        winnt::kill_process(pid)
    }

    #[cfg(unix)]
    pub async fn terminate_gracefully(pid: u32, grace_period_ms: u64) -> bool {
        posix::terminate_gracefully(pid, grace_period_ms).await
    }

    #[cfg(windows)]
    pub async fn terminate_gracefully(pid: u32, grace_period_ms: u64) -> bool {
        winnt::terminate_gracefully(pid, grace_period_ms).await
    }

    #[cfg(unix)]
    pub fn process_exists(pid: u32) -> bool {
        posix::process_exists(pid)
    }

    #[cfg(windows)]
    pub fn process_exists(pid: u32) -> bool {
        winnt::process_exists(pid)
    }

    #[cfg(unix)]
    pub fn kill_by_pattern(pattern: &str) -> usize {
        for protected in PROTECTED_PROCESSES {
            if pattern.contains(protected) {
                return 0;
            }
        }
        posix::kill_by_pattern(pattern)
    }

    #[cfg(windows)]
    pub fn kill_by_pattern(pattern: &str) -> usize {
        for protected in PROTECTED_PROCESSES {
            if pattern.contains(protected) {
                return 0;
            }
        }
        winnt::kill_by_pattern(pattern)
    }

    pub async fn wait_for_port_free(
        port: u16,
        max_retries: u8,
        retry_delay_ms: u64,
    ) -> SchedulerResult<()> {
        for attempt in 1..=max_retries {
            if Self::check_port(port).is_none() {
                return Ok(());
            }

            if attempt < max_retries {
                tokio::time::sleep(tokio::time::Duration::from_millis(retry_delay_ms)).await;
            }
        }

        let message = Self::check_port(port).map_or_else(
            || {
                format!(
                    "Port {} still occupied by unknown process after {} attempts",
                    port, max_retries
                )
            },
            |pid| {
                format!(
                    "Port {} still occupied by PID {} after {} attempts",
                    port, pid, max_retries
                )
            },
        );
        Err(SchedulerError::config_error(message))
    }

    #[cfg(unix)]
    pub fn get_process_by_port(port: u16) -> Option<ProcessInfo> {
        posix::get_process_by_port(port)
    }

    #[cfg(windows)]
    pub fn get_process_by_port(port: u16) -> Option<ProcessInfo> {
        winnt::get_process_by_port(port)
    }
}
