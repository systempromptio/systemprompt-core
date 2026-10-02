//! Port management — detect, kill, and verify availability of agent ports.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod probe;

use std::time::Duration;

use systemprompt_identifiers::AgentName;

use crate::services::agent_orchestration::{OrchestrationError, OrchestrationResult, process};

pub use probe::{ProcessInfo, find_process_using_port, get_process_info};

#[derive(Debug, Copy, Clone)]
pub struct PortService;

impl Default for PortService {
    fn default() -> Self {
        Self::new()
    }
}

impl PortService {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub async fn kill_process_on_port(
        &self,
        port: u16,
        agent_name: &AgentName,
    ) -> OrchestrationResult<bool> {
        let pid = match find_process_using_port(port) {
            Ok(Some(p)) => p,
            Ok(None) => {
                return Ok(false);
            },
            Err(e) => {
                return Err(OrchestrationError::spawn(
                    format!("Failed to check port {port}"),
                    e,
                ));
            },
        };

        if !process::pid_is_agent_child(pid, agent_name) {
            return Err(OrchestrationError::ProcessSpawnFailed(format!(
                "Port {port} is in use by PID {pid}, which is not a verified process of agent \
                 '{agent_name}'. Please free the port manually."
            )));
        }

        tracing::warn!(pid = %pid, port = %port, agent = %agent_name, "Killing orphaned agent process");

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

    pub async fn cleanup_port_if_needed(
        &self,
        port: u16,
        agent_name: &AgentName,
    ) -> OrchestrationResult<()> {
        if !process::is_port_in_use(port) {
            return Ok(());
        }

        match find_process_using_port(port) {
            Ok(Some(pid)) if process::pid_is_agent_child(pid, agent_name) => {
                tracing::warn!(port = %port, pid = %pid, agent = %agent_name, "Port occupied by orphaned agent process");
                self.kill_process_on_port(port, agent_name).await?;
                Ok(())
            },
            Ok(Some(pid)) => {
                let info = get_process_info(pid)
                    .inspect_err(|e| {
                        tracing::trace!(pid = %pid, error = %e, "Failed to get process info for error message");
                    })
                    .ok()
                    .flatten()
                    .map_or_else(|| "unknown".to_owned(), |i| i.command);

                Err(OrchestrationError::ProcessSpawnFailed(format!(
                    "Port {port} is in use by PID {pid}, which is not a verified process of agent \
                     '{agent_name}': {info}\nPlease stop the process manually or choose a \
                     different port."
                )))
            },
            Ok(None) => Err(OrchestrationError::ProcessSpawnFailed(format!(
                "Port {} appears to be in use but process cannot be identified",
                port
            ))),
            Err(e) => Err(OrchestrationError::spawn(
                format!("Failed to check port {port}"),
                e,
            )),
        }
    }

    pub async fn cleanup_agent_ports(
        &self,
        ports: &[(u16, AgentName)],
    ) -> OrchestrationResult<u32> {
        let mut cleaned = 0;

        for (port, agent_name) in ports {
            if process::is_port_in_use(*port) {
                self.cleanup_port_if_needed(*port, agent_name).await?;
                cleaned += 1;
            }
        }

        if cleaned > 0 {
            tracing::info!(cleaned = %cleaned, "Cleaned up ports");
        }

        Ok(cleaned)
    }

    pub fn verify_all_ports_available(ports: &[u16]) -> OrchestrationResult<()> {
        let mut blocked_ports = Vec::new();

        for &port in ports {
            if process::is_port_in_use(port)
                && let Ok(Some(pid)) = find_process_using_port(port)
            {
                blocked_ports.push((port, pid));
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
                        .map_or_else(|| "unknown".to_owned(), |i| i.command);
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
