use anyhow::Result;
use std::sync::Arc;
use systemprompt_mcp::services::registry::RegistryManager;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Phase, StartupEvent, StartupEventExt, StartupEventSender};

pub async fn reconcile_system_services(
    ctx: &AppContext,
    mcp_orchestrator: &Arc<systemprompt_mcp::services::McpManager>,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    if let Some(tx) = events {
        tx.phase_started(Phase::McpServers);
    }

    match cleanup_stale_service_entries(ctx, events).await {
        Ok(count) => {
            if count > 0 {
                if let Some(tx) = events {
                    if tx
                        .unbounded_send(StartupEvent::McpServiceCleanup {
                            name: format!("{count} services"),
                            reason: "Stale entries removed".to_string(),
                        })
                        .is_err()
                    {
                        tracing::debug!("Startup event receiver dropped");
                    }
                }
            }
        },
        Err(e) => {
            if let Some(tx) = events {
                tx.warning(format!("Could not clean stale entries: {e}"));
            }
        },
    }

    let required_servers = RegistryManager::get_enabled_servers()?;
    let required_count = required_servers.len();

    match mcp_orchestrator.reconcile().await {
        Ok(running_count) => {
            handle_reconcile_success(
                running_count,
                required_count,
                &required_servers,
                mcp_orchestrator,
                ctx,
                events,
            )
            .await?;
        },
        Err(e) => {
            if let Some(tx) = events {
                tx.phase_failed(Phase::McpServers, e.to_string());
            }
            return Err(anyhow::anyhow!(
                "FATAL: MCP reconciliation failed: {}\n\nCannot start API without MCP servers.",
                e
            ));
        },
    }

    if let Some(tx) = events {
        tx.phase_completed(Phase::McpServers);
    }
    Ok(())
}

async fn handle_reconcile_success(
    running_count: usize,
    required_count: usize,
    required_servers: &[systemprompt_mcp::McpServerConfig],
    mcp_orchestrator: &Arc<systemprompt_mcp::services::McpManager>,
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    if running_count < required_count {
        return handle_missing_servers(required_servers, mcp_orchestrator, events).await;
    }

    if running_count > 0 {
        verify_database_registration(required_servers, ctx, events).await?;
    }

    if let Some(tx) = events {
        if tx
            .unbounded_send(StartupEvent::McpReconciliationComplete {
                running: running_count,
                required: required_count,
            })
            .is_err()
        {
            tracing::debug!("Startup event receiver dropped");
        }
    }

    Ok(())
}

async fn handle_missing_servers(
    required_servers: &[systemprompt_mcp::McpServerConfig],
    mcp_orchestrator: &Arc<systemprompt_mcp::services::McpManager>,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    let running_servers = mcp_orchestrator.get_running_servers().await?;
    let running_names: std::collections::HashSet<String> =
        running_servers.iter().map(|s| s.name.clone()).collect();

    let missing: Vec<String> = required_servers
        .iter()
        .map(|s| s.name.clone())
        .filter(|name| !running_names.contains(name))
        .collect();

    if let Some(tx) = events {
        if tx
            .unbounded_send(StartupEvent::Error {
                message: format!(
                    "Server status mismatch: {} servers failed to start: {}",
                    missing.len(),
                    missing.join(", ")
                ),
                fatal: true,
            })
            .is_err()
        {
            tracing::debug!("Startup event receiver dropped");
        }
    }

    Err(anyhow::anyhow!(
        "FATAL: {} required MCP server(s) failed to start: {}\n\nSystemPrompt OS cannot operate \
         without MCP servers.\nAgents need tools to function.\n\nBuild missing binaries with:\n  \
         cargo build --bin {}\n\nOr build all MCP servers:\n  just mcp build",
        missing.len(),
        missing.join(", "),
        missing.join(" --bin ")
    ))
}

async fn verify_database_registration(
    required_servers: &[systemprompt_mcp::McpServerConfig],
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    use systemprompt_database::ServiceRepository;
    let service_repo = ServiceRepository::new(ctx.db_pool().clone());

    let mut verification_failed = Vec::new();

    for server in required_servers {
        match service_repo.get_service_by_name(&server.name).await {
            Ok(Some(service)) if service.status == "running" => {
                if let Some(tx) = events {
                    if tx
                        .unbounded_send(StartupEvent::McpServerReady {
                            name: server.name.clone(),
                            port: service.port as u16,
                            startup_time: std::time::Duration::ZERO,
                            tools: 0,
                        })
                        .is_err()
                    {
                        tracing::debug!("Startup event receiver dropped");
                    }
                }
            },
            Ok(Some(service)) => {
                verification_failed.push(format!("{} (status: {})", server.name, service.status));
            },
            Ok(None) => {
                verification_failed.push(format!("{} (not in database)", server.name));
            },
            Err(e) => {
                verification_failed.push(format!("{} (db error: {})", server.name, e));
            },
        }
    }

    if !verification_failed.is_empty() {
        if let Some(tx) = events {
            if tx
                .unbounded_send(StartupEvent::Error {
                    message: format!(
                        "Database verification failed for {} service(s): {}",
                        verification_failed.len(),
                        verification_failed.join(", ")
                    ),
                    fatal: true,
                })
                .is_err()
            {
                tracing::debug!("Startup event receiver dropped");
            }
        }
        return Err(anyhow::anyhow!(
            "FATAL: MCP services running but not properly registered in database\n\nThis \
             indicates a race condition or database synchronization issue.\nFailed services: {}",
            verification_failed.join(", ")
        ));
    }

    Ok(())
}

async fn cleanup_stale_service_entries(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<u64> {
    use systemprompt_database::ServiceRepository;
    use systemprompt_scheduler::ProcessCleanup;

    let repo = ServiceRepository::new(ctx.db_pool().clone());
    let mut deleted_count = 0u64;

    let mcp_services = repo.get_mcp_services().await?;
    for service in mcp_services {
        let should_delete = match service.status.as_str() {
            "running" => service
                .pid
                .is_none_or(|pid| !ProcessCleanup::process_exists(pid as u32)),
            "error" | "stopped" => true,
            _ => false,
        };

        if should_delete && repo.delete_service(&service.name).await.is_ok() {
            deleted_count += 1;
            if let Some(tx) = events {
                if tx
                    .unbounded_send(StartupEvent::McpServiceCleanup {
                        name: service.name.clone(),
                        reason: format!(
                            "Stale entry (status: {}, pid: {:?})",
                            service.status, service.pid
                        ),
                    })
                    .is_err()
                {
                    tracing::debug!("Startup event receiver dropped");
                }
            }
        }
    }

    let agent_service_names = repo.get_all_agent_service_names().await?;
    for service_name in agent_service_names {
        if let Ok(Some(service)) = repo.get_service_by_name(&service_name).await {
            let should_delete = match service.status.as_str() {
                "running" => service
                    .pid
                    .is_none_or(|pid| !ProcessCleanup::process_exists(pid as u32)),
                "error" | "stopped" => true,
                _ => false,
            };

            if should_delete && repo.delete_service(&service_name).await.is_ok() {
                deleted_count += 1;
                if let Some(tx) = events {
                    if tx
                        .unbounded_send(StartupEvent::AgentCleanup {
                            name: service_name.clone(),
                            reason: format!(
                                "Stale entry (status: {}, pid: {:?})",
                                service.status, service.pid
                            ),
                        })
                        .is_err()
                    {
                        tracing::debug!("Startup event receiver dropped");
                    }
                }
            }
        }
    }

    Ok(deleted_count)
}
