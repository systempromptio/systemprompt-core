use super::AgentOrchestrator;
use crate::services::agent_orchestration::{AgentStatus, OrchestrationError, OrchestrationResult};

impl AgentOrchestrator {
    pub async fn delete_agent(&self, agent_id: &str) -> OrchestrationResult<()> {
        tracing::info!(agent_id = %agent_id, "Deleting agent");

        if let Ok(status) = self.get_status(agent_id).await {
            if let AgentStatus::Running { .. } = status {
                tracing::debug!(agent_id = %agent_id, "Stopping running agent before deletion");
                self.lifecycle.disable_agent(agent_id).await?;
            }
        }

        self.db_service.remove_agent_service(agent_id).await?;

        tracing::info!(agent_id = %agent_id, "Agent deleted successfully");
        Ok(())
    }

    pub async fn delete_all_agents(&self) -> OrchestrationResult<u64> {
        tracing::info!("Deleting all agents");

        let agents = self.list_all().await?;
        let total_count = agents.len() as u64;

        if total_count == 0 {
            tracing::debug!("No agents to delete");
            return Ok(0);
        }

        tracing::info!(count = %total_count, "Found agents to delete");

        tracing::debug!("Disabling all running agents");
        self.disable_all().await?;

        let mut deleted_count = 0;
        for (agent_id, _) in agents {
            match self.delete_agent(&agent_id).await {
                Ok(_) => {
                    deleted_count += 1;
                },
                Err(e) => {
                    tracing::error!(agent_id = %agent_id, error = %e, "Failed to delete agent");
                },
            }
        }

        tracing::info!(deleted = %deleted_count, total = %total_count, "Deleted agents");
        Ok(deleted_count)
    }

    pub async fn cleanup_orphaned_processes(&self) -> OrchestrationResult<()> {
        tracing::debug!("Scanning for orphaned agent processes");

        let output = std::process::Command::new("pgrep")
            .arg("-f")
            .arg("agent-worker")
            .output()
            .map_err(|e| {
                OrchestrationError::ProcessSpawnFailed(format!("Failed to run pgrep: {e}"))
            })?;

        if !output.status.success() {
            tracing::debug!("No orphaned agent processes found");
            return Ok(());
        }

        let pids_str = String::from_utf8_lossy(&output.stdout);
        let mut registered = 0;
        let mut failed = 0;

        for line in pids_str.lines() {
            if let Ok(pid) = line.trim().parse::<u32>() {
                if self.is_pid_tracked(pid).await? {
                    continue;
                }

                if let Some((agent_id, port)) = self.identify_orphaned_process(pid).await? {
                    tracing::debug!(
                        pid = %pid,
                        agent_id = %agent_id,
                        port = %port,
                        "Found orphaned process"
                    );

                    let name = self
                        .db_service
                        .get_agent_config(&agent_id)
                        .await
                        .map(|config| config.name)
                        .unwrap_or_else(|_| "unknown".to_string());

                    match self.db_service.register_agent(&name, pid, port).await {
                        Ok(service_id) => {
                            tracing::info!(
                                service_id = %service_id,
                                pid = %pid,
                                "Registered orphaned process as service"
                            );
                            registered += 1;
                        },
                        Err(e) => {
                            tracing::error!(
                                error = %e,
                                pid = %pid,
                                "Failed to register orphaned process"
                            );
                            failed += 1;
                        },
                    }
                } else {
                    tracing::warn!(pid = %pid, "Could not identify agent for orphaned process");
                    failed += 1;
                }
            }
        }

        if registered > 0 {
            tracing::info!(registered = %registered, "Registered orphaned processes");
        }
        if failed > 0 {
            tracing::warn!(failed = %failed, "Failed to handle some processes");
        }

        Ok(())
    }

    pub(super) async fn is_pid_tracked(&self, pid: u32) -> OrchestrationResult<bool> {
        let agents = self.db_service.list_all_agents().await?;
        for (_, status) in agents {
            if let AgentStatus::Running {
                pid: tracked_pid, ..
            } = status
            {
                if tracked_pid == pid {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub(super) async fn identify_orphaned_process(
        &self,
        pid: u32,
    ) -> OrchestrationResult<Option<(String, u16)>> {
        let environ_path = format!("/proc/{}/environ", pid);
        if let Ok(environ_data) = std::fs::read(&environ_path) {
            let environ_str = String::from_utf8_lossy(&environ_data);
            let mut agent_id = None;
            let mut port = None;

            for env_var in environ_str.split('\0') {
                if env_var.starts_with("AGENT_ID=") || env_var.starts_with("AGENT_UUID=") {
                    agent_id = env_var.split('=').nth(1).map(|s| s.to_string());
                } else if env_var.starts_with("AGENT_PORT=") {
                    if let Some(port_str) = env_var.split('=').nth(1) {
                        port = port_str.parse::<u16>().ok();
                    }
                }
            }

            if let (Some(id), Some(p)) = (agent_id, port) {
                return Ok(Some((id, p)));
            }
        }

        Ok(None)
    }
}
