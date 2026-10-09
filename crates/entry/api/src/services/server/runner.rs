//! Server run loop: MCP orchestrator wiring and lifecycle supervision.
//!
//! The startup phases a node runs follow its `server.role`
//! ([`super::routes::role::lifecycle_plan`]): a gateway node spawns no MCP
//! servers or agents and runs no scheduler.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::services::SchedulerHandle;
use systemprompt_traits::{OwnedTask, Phase, StartupEvent, StartupEventExt, StartupEventSender};

use super::lifecycle::{
    initialize_scheduler, reconcile_agents, reconcile_system_services, start_event_bridge,
    start_registry_heartbeat,
};

pub async fn run_server(
    ctx: AppContext,
    events: Option<StartupEventSender>,
    early: super::startup::EarlyServer,
) -> Result<()> {
    let start_time = std::time::Instant::now();

    let instance_claim = ctx
        .service_repository()
        .claim_instance()
        .await
        .context("replica identity")?;
    tracing::info!(instance_id = %instance_claim.instance_id(), "replica identity claimed");

    let plan = super::routes::role::lifecycle_plan(ctx.config().role);
    tracing::info!(role = %ctx.config().role, "node role");

    start_event_bridge(&ctx);
    start_registry_heartbeat(&ctx);
    if plan.reconcile_mcp {
        let mcp_orchestrator = create_mcp_orchestrator(&ctx)?;
        reconcile_system_services(&ctx, &mcp_orchestrator, events.as_ref()).await?;
    }
    if plan.reconcile_agents {
        run_agents_phase(&ctx, events.as_ref()).await?;
    }
    let scheduler_handle = if plan.scheduler {
        run_scheduler_phase(&ctx, events.as_ref()).await?
    } else {
        None
    };

    if let Some(ref tx) = events {
        tx.phase_started(Phase::ApiServer);
    }
    let router = crate::services::server::setup_api_server(&ctx, events.as_ref())?;
    start_accounting_recovery(&ctx).await?;
    let addr = ctx.server_address();

    early.activate(router);
    let metrics_listener = start_metrics_listener(&ctx).await?;
    super::readiness::signal_ready();

    if let Some(ref tx) = events {
        tx.phase_completed(Phase::ApiServer);
    }

    if let Some(ref tx) = events {
        tx.startup_complete(start_time.elapsed(), format!("http://{}", addr), vec![]);
    }

    systemprompt_logging::set_startup_mode(false);

    let restart = ctx.shutdown_request().clone();
    let serve_result = super::shutdown::join_within_drain_grace(early.join(), &restart).await;

    let forced_exit = super::shutdown::arm_forced_exit(restart);
    if let Some(listener) = metrics_listener
        && listener.abort_and_join().await.is_some()
    {
        tracing::debug!("Metrics listener had already stopped on the shutdown signal");
    }
    super::shutdown::drain(&ctx, scheduler_handle).await;
    instance_claim.release().await;
    forced_exit.abort();

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
        (**ctx.service_repository()).clone(),
        Arc::clone(ctx.app_paths_arc()),
        ctx.mcp_registry().clone(),
    )?;
    Ok(Arc::new(manager))
}

async fn start_metrics_listener(ctx: &AppContext) -> Result<Option<OwnedTask<()>>> {
    let Some(port) = ctx.config().metrics_port else {
        return Ok(None);
    };
    let handle = super::metrics::install_recorder(&ctx.config().instance_id)?;
    super::pool_metrics::spawn_sampler(ctx);
    let addr = std::net::SocketAddr::new(ctx.config().host.parse()?, port);
    Ok(Some(
        super::metrics::serve_metrics_listener(addr, handle).await?,
    ))
}

async fn start_accounting_recovery(ctx: &AppContext) -> Result<()> {
    let settlement = crate::routes::gateway::gateway_repositories(ctx)?.settlement();
    let settled = systemprompt_gateway::audit::journal::recover(&settlement).await?;
    if settled > 0 {
        tracing::info!(settled, "Gateway accounting receipts recovered at startup");
    }
    systemprompt_gateway::audit::journal::spawn_recovery(settlement, ctx.background_tasks());
    Ok(())
}
