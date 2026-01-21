use anyhow::Result;
use futures_util::future::join_all;
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{StartupEvent, StartupEventSender};

pub async fn reconcile_agents(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<usize> {
    use systemprompt_agent::services::agent_orchestration::AgentOrchestrator;
    use systemprompt_agent::services::registry::AgentRegistry;

    let orchestrator = match AgentOrchestrator::new(Arc::new(ctx.clone()), events).await {
        Ok(orch) => orch,
        Err(e) => {
            if let Some(tx) = events {
                if tx
                    .unbounded_send(StartupEvent::Error {
                        message: format!("Failed to initialize agent orchestrator: {e}"),
                        fatal: true,
                    })
                    .is_err()
                {
                    tracing::debug!(
                        "Startup event receiver dropped - startup may have been cancelled"
                    );
                }
            }
            return Err(e.into());
        },
    };

    let agent_registry = match AgentRegistry::new().await {
        Ok(registry) => registry,
        Err(e) => {
            if let Some(tx) = events {
                if tx
                    .unbounded_send(StartupEvent::Error {
                        message: format!("Failed to load agent registry: {e}"),
                        fatal: true,
                    })
                    .is_err()
                {
                    tracing::debug!(
                        "Startup event receiver dropped - startup may have been cancelled"
                    );
                }
            }
            return Err(e);
        },
    };

    let enabled_agents = match agent_registry.list_enabled_agents().await {
        Ok(agents) => agents,
        Err(e) => {
            if let Some(tx) = events {
                if tx
                    .unbounded_send(StartupEvent::Error {
                        message: format!("Failed to list enabled agents: {e}"),
                        fatal: true,
                    })
                    .is_err()
                {
                    tracing::debug!(
                        "Startup event receiver dropped - startup may have been cancelled"
                    );
                }
            }
            return Err(e);
        },
    };

    let required_count = enabled_agents.len();
    let orchestrator = &orchestrator;

    let start_futures: Vec<_> = enabled_agents
        .iter()
        .map(|agent_config| {
            let name = agent_config.name.clone();
            let port = agent_config.port;
            async move {
                enforce_clean_agent_state(orchestrator, &name, port, events)
                    .await
                    .map(|_| name.clone())
                    .map_err(|e| (name.clone(), e.to_string()))
            }
        })
        .collect();

    let results = join_all(start_futures).await;

    let (succeeded, failed): (Vec<_>, Vec<_>) = results.into_iter().partition(Result::is_ok);

    let started = succeeded.len();
    let failed_agents: Vec<(String, String)> = failed.into_iter().filter_map(Result::err).collect();

    if !failed_agents.is_empty() {
        let started = handle_failed_agents(
            started,
            &failed_agents,
            &agent_registry,
            orchestrator,
            events,
        )
        .await?;

        if started < required_count {
            return Err(anyhow::anyhow!(
                "FATAL: Only {}/{} required agents started successfully\n\nAll enabled agents \
                 must be running for API to start.",
                started,
                required_count
            ));
        }

        return Ok(started);
    }

    if started < required_count {
        return Err(anyhow::anyhow!(
            "FATAL: Only {}/{} required agents started successfully\n\nAll enabled agents must be \
             running for API to start.",
            started,
            required_count
        ));
    }

    Ok(started)
}

async fn handle_failed_agents(
    mut started: usize,
    failed_agents: &[(String, String)],
    agent_registry: &systemprompt_agent::services::registry::AgentRegistry,
    orchestrator: &systemprompt_agent::services::agent_orchestration::AgentOrchestrator,
    events: Option<&StartupEventSender>,
) -> Result<usize> {
    if let Some(tx) = events {
        if tx
            .unbounded_send(StartupEvent::Warning {
                message: format!(
                    "{} agent(s) failed to start on first attempt",
                    failed_agents.len()
                ),
                context: Some("Attempting cleanup and retry".to_string()),
            })
            .is_err()
        {
            tracing::debug!("Startup event receiver dropped - startup may have been cancelled");
        }
    }

    let mut retry_failed: Vec<(String, String)> = Vec::new();

    for (agent_name, _original_error) in failed_agents {
        let agent_config = match agent_registry.get_agent(agent_name).await {
            Ok(config) => config,
            Err(e) => {
                if let Some(tx) = events {
                    if tx
                        .unbounded_send(StartupEvent::AgentFailed {
                            name: agent_name.clone(),
                            error: format!("Agent config not found: {e}"),
                        })
                        .is_err()
                    {
                        tracing::debug!(
                            "Startup event receiver dropped - startup may have been cancelled"
                        );
                    }
                }
                retry_failed.push((agent_name.clone(), format!("Agent config not found: {e}")));
                continue;
            },
        };

        match enforce_clean_agent_state(orchestrator, agent_name, agent_config.port, events).await {
            Ok(_) => {
                started += 1;
            },
            Err(e) => {
                retry_failed.push((agent_name.clone(), e.to_string()));
            },
        }
    }

    if !retry_failed.is_empty() {
        let agent_names: Vec<String> = retry_failed.iter().map(|(name, _)| name.clone()).collect();
        return Err(anyhow::anyhow!(
            "FATAL: {} required agent(s) failed to start after retry: {}\n\nSystemPrompt OS \
             cannot operate without all enabled agents.\nAgents are the core service \
             layer.\n\nFailures:\n{}\n\nPossible causes:\n  - Agent binaries not built (run: \
             cargo build)\n  - Ports occupied by non-agent processes (check with: lsof -i:PORT)\n  \
             - Missing environment variables (check .env file)\n  - File permission \
             issues\n\nBuild agents with: cargo build",
            retry_failed.len(),
            agent_names.join(", "),
            retry_failed
                .iter()
                .map(|(name, err)| format!("  - {name}: {err}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    Ok(started)
}

async fn enforce_clean_agent_state(
    orchestrator: &systemprompt_agent::services::agent_orchestration::AgentOrchestrator,
    agent_id: &str,
    desired_port: u16,
    events: Option<&StartupEventSender>,
) -> Result<bool> {
    use systemprompt_agent::services::agent_orchestration::{AgentStatus, PortManager};

    if let Ok(status) = orchestrator.get_status(agent_id).await {
        match status {
            AgentStatus::Running { pid, port } => {
                use systemprompt_agent::services::agent_orchestration::process;
                let reason = if port == desired_port {
                    format!("Restarting agent to ensure fresh state (pid {pid})")
                } else {
                    format!(
                        "On wrong port {port} (expected {desired_port}), killing and restarting"
                    )
                };
                if let Some(tx) = events {
                    if tx
                        .unbounded_send(StartupEvent::AgentCleanup {
                            name: agent_id.to_string(),
                            reason,
                        })
                        .is_err()
                    {
                        tracing::debug!(
                            "Startup event receiver dropped - startup may have been cancelled"
                        );
                    }
                }
                process::terminate_gracefully(pid, 5).await.ok();
                orchestrator.delete_agent(agent_id).await.ok();
            },
            AgentStatus::Failed { .. } => {
                if let Some(tx) = events {
                    if tx
                        .unbounded_send(StartupEvent::AgentCleanup {
                            name: agent_id.to_string(),
                            reason: "Previously failed, restarting".to_string(),
                        })
                        .is_err()
                    {
                        tracing::debug!(
                            "Startup event receiver dropped - startup may have been cancelled"
                        );
                    }
                }
            },
        }
    }

    let port_manager = PortManager::new();
    if let Err(e) = port_manager.cleanup_port_if_needed(desired_port).await {
        if let Some(tx) = events {
            if tx
                .unbounded_send(StartupEvent::Error {
                    message: format!(
                        "Failed to cleanup port {desired_port} for agent {agent_id}: {e}"
                    ),
                    fatal: false,
                })
                .is_err()
            {
                tracing::debug!("Startup event receiver dropped - startup may have been cancelled");
            }
        }
        return Err(e.into());
    }

    match orchestrator.start_agent(agent_id, events).await {
        Ok(_) => Ok(true),
        Err(e) => Err(e.into()),
    }
}
