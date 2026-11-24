//! MCP service reconciliation during server startup.
//!
//! `reconcile_system_services` cleans stale service rows, then has the `mcp`
//! member's process set reconcile its servers to the required set and verify
//! each is registered and running, failing server startup loudly if any
//! required MCP server is missing, since agents depend on their tools. With
//! no MCP process set admitted, startup is refused the same way.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::Result;
use systemprompt_extension::service_process::ServiceProcessSets;
use systemprompt_loader::subprocess::SubprocessLiveness;
use systemprompt_manifest::services::ServiceModule;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{
    Phase, ServiceProcessError, ServiceProcessKind, StartupEventExt, StartupEventSender,
};

pub(in crate::services::server) async fn reconcile_system_services(
    ctx: &AppContext,
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

    let kind = ServiceProcessKind::McpServer;
    let sets = ctx
        .kernel()
        .state()
        .find::<Arc<ServiceProcessSets>>()
        .cloned()
        .unwrap_or_default();
    let set = sets
        .of_kind(kind)
        .ok_or(ServiceProcessError::Unavailable { kind })?;
    set.reconcile(events).await?;

    events.phase_completed(Phase::McpServers);
    Ok(())
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
