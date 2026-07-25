//! Server run loop: MCP orchestrator wiring and lifecycle supervision.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::services::SchedulerHandle;
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

    run_agents_phase(&ctx, events.as_ref()).await?;
    let scheduler_handle = run_scheduler_phase(&ctx, events.as_ref()).await?;

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

async fn run_agents_phase(ctx: &AppContext, events: Option<&StartupEventSender>) -> Result<()> {
    if let Some(tx) = events {
        tx.phase_started(Phase::Agents);
    }
    match reconcile_agents(ctx, events).await {
        Ok(started_count) => {
            if let Some(tx) = events {
                send_startup_event(
                    tx,
                    StartupEvent::AgentReconciliationComplete {
                        running: started_count,
                        total: started_count,
                    },
                );
                tx.phase_completed(Phase::Agents);
            }
            Ok(())
        },
        Err(e) => Err(fail_phase(
            events,
            Phase::Agents,
            format!("Agent reconciliation failed: {e}"),
            e,
        )),
    }
}

/// A scheduler that cannot start is fatal: `run_bootstrap_jobs` is reached only
/// through this phase, so continuing would serve a process whose boot-time jobs
/// silently never ran. Disabling the scheduler is done via
/// `scheduler.enabled: false`, which succeeds here with no handle.
async fn run_scheduler_phase(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<Option<SchedulerHandle>> {
    if let Some(tx) = events {
        tx.phase_started(Phase::Scheduler);
    }
    match initialize_scheduler(ctx, events).await {
        Ok(handle) => {
            if let Some(tx) = events {
                tx.phase_completed(Phase::Scheduler);
            }
            Ok(handle)
        },
        Err(e) => Err(fail_phase(
            events,
            Phase::Scheduler,
            format!("Scheduler initialization failed: {e}"),
            e,
        )),
    }
}

fn fail_phase(
    events: Option<&StartupEventSender>,
    phase: Phase,
    message: String,
    error: anyhow::Error,
) -> anyhow::Error {
    if let Some(tx) = events {
        tx.phase_failed(phase, error.to_string());
        send_startup_event(
            tx,
            StartupEvent::Error {
                message,
                fatal: true,
            },
        );
    }
    error
}

fn send_startup_event(tx: &StartupEventSender, event: StartupEvent) {
    if tx.unbounded_send(event).is_err() {
        tracing::debug!("Startup event receiver dropped");
    }
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
