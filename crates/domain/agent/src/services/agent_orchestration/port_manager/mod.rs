//! Port management — detect, kill, and verify availability of agent ports.

mod probe;

use std::time::Duration;

use crate::services::agent_orchestration::{OrchestrationError, OrchestrationResult, process};

pub use probe::{ProcessInfo, find_process_using_port, get_process_info, is_agent_process};

/// Helper for managing the TCP ports allocated to agent worker processes.
#[derive(Debug, Copy, Clone)]
pub struct PortManager;

impl Default for PortManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PortManager {
    /// Construct a new `PortManager`.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Kill the agent process bound to `port`, if any. Refuses to kill
    /// non-agent processes.
    ///
    /// # Errors
    /// Returns [`OrchestrationError::ProcessSpawnFailed`] if the port lookup or
    /// kill fails, or if the bound process is not an agent.
    pub async fn kill_process_on_port(&self, port: u16) -> OrchestrationResult<bool> {
        let pid = match find_process_using_port(port) {
            Ok(Some(p)) => p,
            Ok(None) => {
                return Ok(false);
            },
            Err(e) => {
                return Err(OrchestrationError::ProcessSpawnFailed(format!(
                    "Failed to check port {}: {}",
                    port, e
                )));
            },
        };

        match is_agent_process(pid) {
            Ok(true) => {},
            Ok(false) => {
                return Err(OrchestrationError::ProcessSpawnFailed(format!(
                    "Port {} is in use by non-agent process (PID {}). Please free the port \
                     manually.",
                    port, pid
                )));
            },
            Err(e) => {
                return Err(OrchestrationError::ProcessSpawnFailed(format!(
                    "Port {} is in use but failed to identify process (PID {}): {}",
                    port, pid, e
                )));
            },
        }

        tracing::warn!(pid = %pid, port = %port, "Killing orphaned agent process");

        if !process::kill_process(pid) {
            return Err(OrchestrationError::ProcessSpawnFailed(format!(
                "Failed to kill process {} on port {}",
                pid, port
            )));
        }

        self.wait_for_port_available(port, 5).await?;

        tracing::debug!(port = %port, "Port is now available");
        Ok(true)
    }

    /// Poll until `port` becomes free, up to `timeout_secs` seconds.
    ///
    /// # Errors
    /// Returns [`OrchestrationError::ProcessSpawnFailed`] if the timeout
    /// elapses.
    pub async fn wait_for_port_available(
        &self,
        port: u16,
        timeout_secs: u64,
    ) -> OrchestrationResult<()> {
        let check_interval = Duration::from_millis(100);
        let max_checks = (timeout_secs * 1000) / 100;

        for _ in 0..max_checks {
            if !process::is_port_in_use(port) {
                return Ok(());
            }
            tokio::time::sleep(check_interval).await;
        }

        Err(OrchestrationError::ProcessSpawnFailed(format!(
            "Port {} did not become available within {} seconds",
            port, timeout_secs
        )))
    }

    /// Free `port` by killing any orphaned agent process bound to it.
    ///
    /// # Errors
    /// Returns [`OrchestrationError::ProcessSpawnFailed`] when the bound
    /// process is not an agent, or any underlying probe/kill operation
    /// fails.
    pub async fn cleanup_port_if_needed(&self, port: u16) -> OrchestrationResult<()> {
        if !process::is_port_in_use(port) {
            return Ok(());
        }

        match find_process_using_port(port) {
            Ok(Some(pid)) => match is_agent_process(pid) {
                Ok(true) => {
                    tracing::warn!(port = %port, pid = %pid, "Port occupied by orphaned agent process");
                    self.kill_process_on_port(port).await?;
                },
                Ok(false) => {
                    let info = get_process_info(pid)
                        .map_err(|e| {
                            tracing::trace!(pid = %pid, error = %e, "Failed to get process info for error message");
                            e
                        })
                        .ok()
                        .flatten()
                        .map_or_else(|| "unknown".to_string(), |i| i.command);

                    return Err(OrchestrationError::ProcessSpawnFailed(format!(
                        "Port {} is in use by non-agent process (PID {}): {}\nPlease stop the \
                         process manually or choose a different port.",
                        port, pid, info
                    )));
                },
                Err(e) => {
                    return Err(OrchestrationError::ProcessSpawnFailed(format!(
                        "Port {} is in use but failed to identify process (PID {}): {}",
                        port, pid, e
                    )));
                },
            },
            Ok(None) => {
                return Err(OrchestrationError::ProcessSpawnFailed(format!(
                    "Port {} appears to be in use but process cannot be identified",
                    port
                )));
            },
            Err(e) => {
                return Err(OrchestrationError::ProcessSpawnFailed(format!(
                    "Failed to check port {}: {}",
                    port, e
                )));
            },
        }

        Ok(())
    }

    /// Free every port in `ports` that is currently held by an orphaned agent.
    ///
    /// # Errors
    /// Returns [`OrchestrationError::ProcessSpawnFailed`] from
    /// [`Self::cleanup_port_if_needed`] for the first failing port.
    pub async fn cleanup_agent_ports(&self, ports: &[u16]) -> OrchestrationResult<u32> {
        let mut cleaned = 0;

        for &port in ports {
            if process::is_port_in_use(port) {
                match self.cleanup_port_if_needed(port).await {
                    Ok(()) => cleaned += 1,
                    Err(e) => {
                        tracing::error!(port = %port, error = %e, "Failed to cleanup port");
                        return Err(e);
                    },
                }
            }
        }

        if cleaned > 0 {
            tracing::info!(cleaned = %cleaned, "Cleaned up ports");
        }

        Ok(cleaned)
    }

    /// Verify that every port in `ports` is currently free.
    ///
    /// # Errors
    /// Returns [`OrchestrationError::ProcessSpawnFailed`] listing each blocked
    /// port with the offending PID and command line.
    pub fn verify_all_ports_available(ports: &[u16]) -> OrchestrationResult<()> {
        let mut blocked_ports = Vec::new();

        for &port in ports {
            if process::is_port_in_use(port) {
                if let Ok(Some(pid)) = find_process_using_port(port) {
                    blocked_ports.push((port, pid));
                }
            }
        }

        if !blocked_ports.is_empty() {
            let port_info: Vec<String> = blocked_ports
                .iter()
                .map(|(port, pid)| {
                    let info = get_process_info(*pid)
                        .map_err(|e| {
                            tracing::trace!(pid = %pid, error = %e, "Failed to get process info for port status");
                            e
                        })
                        .ok()
                        .flatten()
                        .map_or_else(|| "unknown".to_string(), |i| i.command);
                    format!("  • Port {} - PID {} ({})", port, pid, info)
                })
                .collect();

            return Err(OrchestrationError::ProcessSpawnFailed(format!(
                "The following ports are still in use:\n{}",
                port_info.join("\n")
            )));
        }

        Ok(())
    }
}
