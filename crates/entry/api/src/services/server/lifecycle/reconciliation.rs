//! MCP service reconciliation during server startup.
//!
//! [`reconcile_system_services`] cleans stale service rows, reconciles the MCP
//! orchestrator to the required set of enabled servers, and verifies each is
//! registered and running in the database — failing server startup loudly if
//! any required MCP server is missing, since agents depend on their tools.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
    reason = "`events` is consumed by `OptionalStartupEventExt` trait methods \
              (`events.error(...)`); clippy's `collection_is_never_read` heuristic does not \
              recognise those calls as reads of the `Option`"
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
    reason = "`events` is consumed by OptionalStartupEventExt trait methods that clippy does not \
              recognise as reads"
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
        match service_repo.find_service_by_name(&server.name).await {
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
    reason = "`events` is consumed by OptionalStartupEventExt trait methods that clippy does not \
              recognise as reads"
)]
async fn cleanup_stale_service_entries(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<u64> {
    use systemprompt_database::ServiceRepository;
    use systemprompt_models::subprocess::{AGENT_NAME_ENV, MCP_SERVICE_ID_ENV};

    let repo = ServiceRepository::new(ctx.db_pool())?;
    let mut deleted_count = 0u64;

    let mcp_services = repo.list_mcp_services().await?;
    for service in mcp_services {
        if !service_row_is_stale(
            service.status.as_str(),
            service.pid,
            MCP_SERVICE_ID_ENV,
            &service.name,
        ) {
            continue;
        }
        if repo.delete_service(&service.name).await.is_ok() {
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

    let agent_service_names = repo.list_all_agent_service_names().await?;
    for service_name in agent_service_names {
        if let Ok(Some(service)) = repo.find_service_by_name(&service_name).await {
            if !service_row_is_stale(
                service.status.as_str(),
                service.pid,
                AGENT_NAME_ENV,
                &service_name,
            ) {
                continue;
            }
            if repo.delete_service(&service_name).await.is_ok() {
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

#[cfg(feature = "test-api")]
pub mod test_api {
    use anyhow::Result;
    use systemprompt_runtime::AppContext;

    #[must_use]
    pub fn service_row_is_stale(
        status: &str,
        pid: Option<i32>,
        name_key: &str,
        name: &str,
    ) -> bool {
        super::service_row_is_stale(status, pid, name_key, name)
    }

    pub async fn cleanup_stale_service_entries(ctx: &AppContext) -> Result<u64> {
        super::cleanup_stale_service_entries(ctx, None).await
    }
}

/// A `running` row is stale unless its recorded PID is alive *and* still names
/// our child — a recycled PID that now belongs to an unrelated process must be
/// dropped, never adopted (and never signalled on the next reap). `error` /
/// `stopped` rows are always stale; any other status is left untouched.
fn service_row_is_stale(status: &str, pid: Option<i32>, name_key: &str, name: &str) -> bool {
    use systemprompt_scheduler::ProcessCleanup;

    match status {
        "running" => {
            let Some(pid) = pid.and_then(|p| u32::try_from(p).ok()) else {
                return true;
            };
            if !ProcessCleanup::process_exists(pid) {
                return true;
            }
            !systemprompt_models::subprocess::live_pid_is_subprocess(pid, name_key, name)
        },
        "error" | "stopped" => true,
        _ => false,
    }
}
