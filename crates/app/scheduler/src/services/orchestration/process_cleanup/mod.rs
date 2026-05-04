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

/// Process and port lifecycle helper. All methods are static — the type is a
/// zero-sized marker carried purely for API ergonomics.
#[derive(Debug, Clone, Copy)]
pub struct ProcessCleanup;

/// Snapshot of a process bound to a TCP port.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Operating-system process identifier.
    pub pid: u32,
    /// Process / executable name as reported by the OS.
    pub name: String,
    /// TCP port the process is bound to at observation time.
    pub port: u16,
}

impl ProcessCleanup {
    /// Return the PID currently bound to the supplied TCP port, or `None` if
    /// the port is free or protected.
    #[cfg(unix)]
    pub fn check_port(port: u16) -> Option<u32> {
        if PROTECTED_PORTS.contains(&port) {
            return None;
        }
        posix::check_port(port)
    }

    /// Return the PID currently bound to the supplied TCP port, or `None` if
    /// the port is free or protected.
    #[cfg(windows)]
    pub fn check_port(port: u16) -> Option<u32> {
        if PROTECTED_PORTS.contains(&port) {
            return None;
        }
        winnt::check_port(port)
    }

    /// Force-kill any non-protected process bound to the supplied port,
    /// returning the list of PIDs that were terminated.
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

    /// Force-kill the supplied PID. Returns `true` if the OS confirmed the
    /// kill request succeeded.
    #[cfg(unix)]
    pub fn kill_process(pid: u32) -> bool {
        posix::kill_process(pid)
    }

    /// Force-kill the supplied PID. Returns `true` if the OS confirmed the
    /// kill request succeeded.
    #[cfg(windows)]
    pub fn kill_process(pid: u32) -> bool {
        winnt::kill_process(pid)
    }

    /// Send a graceful termination signal then wait `grace_period_ms` before
    /// upgrading to a forced kill if the process is still alive.
    #[cfg(unix)]
    pub async fn terminate_gracefully(pid: u32, grace_period_ms: u64) -> bool {
        posix::terminate_gracefully(pid, grace_period_ms).await
    }

    /// Send a graceful termination signal then wait `grace_period_ms` before
    /// upgrading to a forced kill if the process is still alive.
    #[cfg(windows)]
    pub async fn terminate_gracefully(pid: u32, grace_period_ms: u64) -> bool {
        winnt::terminate_gracefully(pid, grace_period_ms).await
    }

    /// Return whether the supplied PID corresponds to a live process.
    #[cfg(unix)]
    pub fn process_exists(pid: u32) -> bool {
        posix::process_exists(pid)
    }

    /// Return whether the supplied PID corresponds to a live process.
    #[cfg(windows)]
    pub fn process_exists(pid: u32) -> bool {
        winnt::process_exists(pid)
    }

    /// Force-kill every process whose command-line matches `pattern`,
    /// excluding the protected-process list. Returns the count of kill
    /// operations that the OS reported succeeded.
    #[cfg(unix)]
    pub fn kill_by_pattern(pattern: &str) -> usize {
        for protected in PROTECTED_PROCESSES {
            if pattern.contains(protected) {
                return 0;
            }
        }
        posix::kill_by_pattern(pattern)
    }

    /// Force-kill every process whose command-line matches `pattern`,
    /// excluding the protected-process list. Returns the count of kill
    /// operations that the OS reported succeeded.
    #[cfg(windows)]
    pub fn kill_by_pattern(pattern: &str) -> usize {
        for protected in PROTECTED_PROCESSES {
            if pattern.contains(protected) {
                return 0;
            }
        }
        winnt::kill_by_pattern(pattern)
    }

    /// Poll the supplied port until it is free, or fail with a typed error
    /// after `max_retries` attempts spaced `retry_delay_ms` apart.
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

    /// Resolve the [`ProcessInfo`] for the process bound to `port`, if any.
    #[cfg(unix)]
    pub fn get_process_by_port(port: u16) -> Option<ProcessInfo> {
        posix::get_process_by_port(port)
    }

    /// Resolve the [`ProcessInfo`] for the process bound to `port`, if any.
    #[cfg(windows)]
    pub fn get_process_by_port(port: u16) -> Option<ProcessInfo> {
        winnt::get_process_by_port(port)
    }
}
