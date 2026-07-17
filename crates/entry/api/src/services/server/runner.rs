//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Phase, StartupEvent, StartupEventExt, StartupEventSender};

use super::lifecycle::{
    initialize_scheduler, reconcile_agents, reconcile_system_services, start_event_bridge,
};

pub async fn run_server(
    ctx: AppContext,
    events: Option<StartupEventSender>,
    early: super::startup::EarlyServer,
) -> Result<()> {
    let start_time = std::time::Instant::now();

    let mcp_orchestrator = create_mcp_orchestrator(&ctx)?;

    start_event_bridge(&ctx);
    reconcile_system_services(&ctx, &mcp_orchestrator, events.as_ref()).await?;

    if let Some(ref tx) = events {
        tx.phase_started(Phase::Agents);
    }
    match reconcile_agents(&ctx, events.as_ref()).await {
        Ok(started_count) => {
            if let Some(ref tx) = events {
                if tx
                    .unbounded_send(StartupEvent::AgentReconciliationComplete {
                        running: started_count,
                        total: started_count,
                    })
                    .is_err()
                {
                    tracing::debug!("Startup event receiver dropped");
                }
                tx.phase_completed(Phase::Agents);
            }
        },
        Err(e) => {
            if let Some(ref tx) = events {
                tx.phase_failed(Phase::Agents, e.to_string());
                if tx
                    .unbounded_send(StartupEvent::Error {
                        message: format!("Agent reconciliation failed: {e}"),
                        fatal: true,
                    })
                    .is_err()
                {
                    tracing::debug!("Startup event receiver dropped");
                }
            }
            return Err(e);
        },
    }

    if let Some(ref tx) = events {
        tx.phase_started(Phase::Scheduler);
    }
    let scheduler_handle = match initialize_scheduler(&ctx, events.as_ref()).await {
        Ok(handle) => {
            if let Some(ref tx) = events {
                tx.phase_completed(Phase::Scheduler);
            }
            handle
        },
        Err(e) => {
            if let Some(ref tx) = events {
                tx.phase_failed(Phase::Scheduler, e.to_string());
            }
            None
        },
    };

    if let Some(ref tx) = events {
        tx.phase_started(Phase::ApiServer);
    }
    let router = crate::services::server::setup_api_server(&ctx, events.as_ref())?;
    let addr = ctx.server_address();

    early.activate(router);
    super::readiness::signal_ready();

    if let Some(ref tx) = events {
        tx.phase_completed(Phase::ApiServer);
    }

    if let Some(ref tx) = events {
        tx.startup_complete(start_time.elapsed(), format!("http://{}", addr), vec![]);
    }

    systemprompt_logging::set_startup_mode(false);

    let serve_result = early.join().await;

    super::shutdown::drain(&ctx, scheduler_handle).await;

    serve_result
}

fn create_mcp_orchestrator(
    ctx: &AppContext,
) -> Result<Arc<systemprompt_mcp::services::McpOrchestrator>> {
    use systemprompt_mcp::services::McpOrchestrator;
    let manager = McpOrchestrator::new(
        Arc::clone(ctx.db_pool()),
        Arc::clone(ctx.app_paths_arc()),
        ctx.mcp_registry().clone(),
    )?;
    Ok(Arc::new(manager))
}
