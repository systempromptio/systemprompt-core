//! MCP service reconciliation during server startup.
//!
//! `reconcile_system_services` cleans stale service rows, reconciles the MCP
//! orchestrator to the required set of enabled servers, and verifies each is
//! registered and running in the database — failing server startup loudly if
//! any required MCP server is missing, since agents depend on their tools.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use std::sync::Arc;
use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::SubprocessLiveness;
use systemprompt_manifest::services::{ServiceModule, ServiceStatus};
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Phase, StartupEventExt, StartupEventSender};

struct ReconcileSuccessParams<'a> {
    running_count: usize,
    required_count: usize,
    required_servers: &'a [systemprompt_mcp::McpServerConfig],
    mcp_orchestrator: &'a Arc<systemprompt_mcp::services::McpOrchestrator>,
    ctx: &'a AppContext,
    events: Option<&'a StartupEventSender>,
}

pub(in crate::services::server) async fn reconcile_system_services(
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

    let required_servers = ctx.mcp_registry().get_managed_servers()?;
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
    reason = "`events` is consumed by `StartupEventExt` trait methods \
              (`events.error(...)`); clippy's `collection_is_never_read` heuristic does not \
              recognise those calls as reads of the `Option`"
)]
pub async fn handle_missing_servers(
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
         with:\n  cargo build --bin {}\n\nOr build all MCP servers:\n  systemprompt build mcp",
        missing.len(),
        missing.join(", "),
        missing.join(" --bin ")
    ))
}

pub const VERIFY_ATTEMPTS: u32 = 5;
pub const VERIFY_BACKOFF: std::time::Duration = std::time::Duration::from_millis(250);

pub async fn verify_database_registration(
    required_servers: &[systemprompt_mcp::McpServerConfig],
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    let mut pending: Vec<&systemprompt_mcp::McpServerConfig> = required_servers.iter().collect();
    let mut failures = Vec::new();

    for attempt in 1..=VERIFY_ATTEMPTS {
        let (still_pending, attempt_failures) = verify_once(&pending, ctx, events).await;
        pending = still_pending;
        failures = attempt_failures;
        if pending.is_empty() {
            return Ok(());
        }
        if attempt < VERIFY_ATTEMPTS {
            tokio::time::sleep(VERIFY_BACKOFF).await;
        }
    }

    events.error(
        format!(
            "Database verification failed for {} service(s): {}",
            failures.len(),
            failures.join(", ")
        ),
        true,
    );
    Err(anyhow::anyhow!(
        "FATAL: MCP services running but not properly registered in database after {} \
         attempts\n\nThis indicates a race condition or database synchronization \
         issue.\nFailed services: {}",
        VERIFY_ATTEMPTS,
        failures.join(", ")
    ))
}

#[expect(
    clippy::collection_is_never_read,
    reason = "`events` is consumed by StartupEventExt trait methods that clippy does not \
              recognise as reads"
)]
async fn verify_once<'a>(
    servers: &[&'a systemprompt_mcp::McpServerConfig],
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> (Vec<&'a systemprompt_mcp::McpServerConfig>, Vec<String>) {
    let service_repo = ctx.service_repository();
    let mut pending = Vec::new();
    let mut failures = Vec::new();

    for &server in servers {
        let failure = match service_repo
            .find_service_by_name(&ServiceName::new(server.name.as_str()))
            .await
        {
            Ok(Some(service)) if service.status == ServiceStatus::Running => {
                events.mcp_ready(
                    server.name.clone(),
                    service.port as u16,
                    std::time::Duration::ZERO,
                    None,
                );
                continue;
            },
            Ok(Some(service)) => format!("{} (status: {})", server.name, service.status),
            Ok(None) => format!("{} (not in database)", server.name),
            Err(e) => format!("{} (db error: {})", server.name, e),
        };
        pending.push(server);
        failures.push(failure);
    }

    (pending, failures)
}

#[expect(
    clippy::collection_is_never_read,
    reason = "`events` is consumed by StartupEventExt trait methods that clippy does not \
              recognise as reads"
)]
pub async fn cleanup_stale_service_entries(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<u64> {
    let reaped = ctx
        .service_repository()
        .reap_stale(&SubprocessLiveness)
        .await?;
    for dead in &reaped.dead {
        let reason = format!("Stale entry (status: running, pid: {})", dead.pid);
        match dead.module_name {
            ServiceModule::Mcp => events.mcp_service_cleanup(dead.name.as_str(), reason),
            ServiceModule::Agent => events.agent_cleanup(dead.name.as_str(), reason),
        }
    }
    Ok(reaped.total())
}
