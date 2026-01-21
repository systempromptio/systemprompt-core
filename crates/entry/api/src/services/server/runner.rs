use anyhow::Result;
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Phase, StartupEvent, StartupEventExt, StartupEventSender};

use super::lifecycle::{initialize_scheduler, reconcile_agents, reconcile_system_services};

pub async fn run_server(ctx: AppContext, events: Option<StartupEventSender>) -> Result<()> {
    let start_time = std::time::Instant::now();

    let mcp_orchestrator = create_mcp_orchestrator(&ctx)?;
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
    match initialize_scheduler(&ctx, events.as_ref()).await {
        Ok(()) => {
            if let Some(ref tx) = events {
                tx.phase_completed(Phase::Scheduler);
            }
        },
        Err(e) => {
            if let Some(ref tx) = events {
                tx.phase_failed(Phase::Scheduler, e.to_string());
            }
        },
    }

    if let Some(ref tx) = events {
        tx.phase_started(Phase::ApiServer);
    }
    let api_server = crate::services::server::setup_api_server(&ctx, events.clone())?;
    let addr = ctx.server_address();

    if let Some(ref tx) = events {
        tx.phase_completed(Phase::ApiServer);
    }

    if let Some(ref tx) = events {
        tx.startup_complete(start_time.elapsed(), format!("http://{}", addr), vec![]);
    }

    systemprompt_logging::set_startup_mode(false);

    api_server.serve(&addr).await
}

fn create_mcp_orchestrator(
    ctx: &AppContext,
) -> Result<Arc<systemprompt_mcp::services::McpManager>> {
    use systemprompt_mcp::services::McpManager;
    let manager = McpManager::new(ctx.db_pool().clone())?;
    Ok(Arc::new(manager))
}
