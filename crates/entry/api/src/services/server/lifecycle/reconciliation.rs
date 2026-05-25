use anyhow::Result;
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{OptionalStartupEventExt, Phase, StartupEventSender};

struct ReconcileSuccessParams<'a> {
    running_count: usize,
    required_count: usize,
    required_servers: &'a [systemprompt_mcp::McpServerConfig],
    mcp_orchestrator: &'a Arc<systemprompt_mcp::services::McpOrchestrator>,
    ctx: &'a AppContext,
    events: Option<&'a StartupEventSender>,
}

pub(crate) async fn reconcile_system_services(
    ctx: &AppContext,
    mcp_orchestrator: &Arc<systemprompt_mcp::services::McpOrchestrator>,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    events.phase_started(Phase::McpServers);

    match cleanup_stale_service_entries(ctx, events).await {
        Ok(count) => {
            if count > 0 {
                events.mcp_service_cleanup(format!("{count} services"), "Stale entries removed");
            }
        },
        Err(e) => {
            events.warning(format!("Could not clean stale entries: {e}"));
        },
    }

    let required_servers = ctx.mcp_registry().get_enabled_servers()?;
    let required_count = required_servers.len();

    match mcp_orchestrator.reconcile().await {
        Ok(running_count) => {
            handle_reconcile_success(ReconcileSuccessParams {
                running_count,
                required_count,
                required_servers: &required_servers,
                mcp_orchestrator,
                ctx,
                events,
            })
            .await?;
        },
        Err(e) => {
            events.phase_failed(Phase::McpServers, e.to_string());
            return Err(anyhow::anyhow!(
                "FATAL: MCP reconciliation failed: {}\n\nCannot start API without MCP servers.",
                e
            ));
        },
    }

    events.phase_completed(Phase::McpServers);
    Ok(())
}

async fn handle_reconcile_success(params: ReconcileSuccessParams<'_>) -> Result<()> {
    if params.running_count < params.required_count {
        return handle_missing_servers(
            params.required_servers,
            params.mcp_orchestrator,
            params.events,
        )
        .await;
    }

    if params.running_count > 0 {
        verify_database_registration(params.required_servers, params.ctx, params.events).await?;
    }

    params
        .events
        .mcp_reconciliation_complete(params.running_count, params.required_count);

    Ok(())
}

#[expect(
    clippy::collection_is_never_read,
    reason = "`events: Option<StartupEventSender>` is consumed through `OptionalStartupEventExt` trait calls (`events.error(...)`); clippy's heuristic does not recognise those as reads"
)]
async fn handle_missing_servers(
    required_servers: &[systemprompt_mcp::McpServerConfig],
    mcp_orchestrator: &Arc<systemprompt_mcp::services::McpOrchestrator>,
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

    events.error(
        format!(
            "Server status mismatch: {} servers failed to start: {}",
            missing.len(),
            missing.join(", ")
        ),
        true,
    );

    Err(anyhow::anyhow!(
        "FATAL: {} required MCP server(s) failed to start: {}\n\nsystemprompt.io OS cannot \
         operate without MCP servers.\nAgents need tools to function.\n\nBuild missing binaries \
         with:\n  cargo build --bin {}\n\nOr build all MCP servers:\n  just mcp build",
        missing.len(),
        missing.join(", "),
        missing.join(" --bin ")
    ))
}

#[expect(
    clippy::collection_is_never_read,
    reason = "`events: Option<...>` is consumed through `OptionalStartupEventExt` trait calls; clippy's heuristic does not see those as reads"
)]
async fn verify_database_registration(
    required_servers: &[systemprompt_mcp::McpServerConfig],
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    use systemprompt_database::ServiceRepository;
    let service_repo = ServiceRepository::new(ctx.db_pool())?;

    let mut verification_failed = Vec::new();

    for server in required_servers {
        match service_repo.get_service_by_name(&server.name).await {
            Ok(Some(service)) if service.status == "running" => {
                events.mcp_ready(
                    server.name.clone(),
                    service.port as u16,
                    std::time::Duration::ZERO,
                    0,
                );
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
        events.error(
            format!(
                "Database verification failed for {} service(s): {}",
                verification_failed.len(),
                verification_failed.join(", ")
            ),
            true,
        );
        return Err(anyhow::anyhow!(
            "FATAL: MCP services running but not properly registered in database\n\nThis \
             indicates a race condition or database synchronization issue.\nFailed services: {}",
            verification_failed.join(", ")
        ));
    }

    Ok(())
}

#[expect(
    clippy::collection_is_never_read,
    reason = "`events: Option<...>` is consumed through `OptionalStartupEventExt` trait calls; clippy's heuristic does not see those as reads"
)]
async fn cleanup_stale_service_entries(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<u64> {
    use systemprompt_database::ServiceRepository;
    use systemprompt_scheduler::ProcessCleanup;

    let repo = ServiceRepository::new(ctx.db_pool())?;
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
            events.mcp_service_cleanup(
                service.name.clone(),
                format!(
                    "Stale entry (status: {}, pid: {:?})",
                    service.status, service.pid
                ),
            );
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
                events.agent_cleanup(
                    service_name.clone(),
                    format!(
                        "Stale entry (status: {}, pid: {:?})",
                        service.status, service.pid
                    ),
                );
            }
        }
    }

    Ok(deleted_count)
}
