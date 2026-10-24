//! Server run loop: MCP orchestrator wiring and lifecycle supervision.
//!
//! The startup phases a node runs follow its `server.role`
//! ([`super::routes::role::lifecycle_plan`]): a gateway node spawns no MCP
//! servers or agents and runs no scheduler; a runner node does neither and
//! instead starts every extension's [`systemprompt_traits::RunnerService`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_identifiers::Actor;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::services::SchedulerHandle;
use systemprompt_traits::{
    Dependencies, JobContext, OwnedTask, Phase, StartupEvent, StartupEventExt, StartupEventSender,
};

use super::lifecycle::{
    initialize_scheduler, reconcile_agents, reconcile_system_services, start_event_bridge,
    start_governance_watch, start_registry_heartbeat, start_schema_watch, start_secrets_refresh,
};
use super::readiness::ReadinessState;
use super::routes::role::LifecyclePlan;
use super::scheduler_health::SchedulerHealth;

pub async fn run_server(
    ctx: AppContext,
    events: Option<StartupEventSender>,
    early: super::startup::EarlyServer,
) -> Result<()> {
    let start_time = std::time::Instant::now();

    let instance_claim = super::replica_identity::claim(&ctx)
        .await
        .context("replica identity")?;
    tracing::info!(instance_id = %instance_claim.instance_id(), "replica identity claimed");

    let plan = super::routes::role::lifecycle_plan(ctx.config().role);
    tracing::info!(role = %ctx.config().role, "node role");

    let scheduler_handle =
        start_role_phases(&ctx, plan, events.as_ref(), early.scheduler_health()).await?;

    if let Some(ref tx) = events {
        tx.phase_started(Phase::ApiServer);
    }
    let router =
        crate::services::server::setup_api_server(&ctx, events.as_ref(), early.readiness())?
            .layer(axum::Extension(Arc::clone(early.scheduler_health())));
    start_accounting_recovery(&ctx).await?;
    let addr = ctx.server_address();

    early.activate(router);
    let metrics_listener = start_metrics_listener(&ctx, plan.scheduler, early.readiness()).await?;
    early.readiness().signal_ready();

    if let Some(ref tx) = events {
        tx.phase_completed(Phase::ApiServer);
    }

    if let Some(ref tx) = events {
        tx.startup_complete(start_time.elapsed(), format!("http://{}", addr), vec![]);
    }

    systemprompt_logging::set_startup_mode(false);

    serve_until_shutdown(
        &ctx,
        early,
        metrics_listener,
        scheduler_handle,
        instance_claim,
    )
    .await
}

async fn start_role_phases(
    ctx: &AppContext,
    plan: LifecyclePlan,
    events: Option<&StartupEventSender>,
    scheduler_health: &SchedulerHealth,
) -> Result<Option<SchedulerHandle>> {
    start_event_bridge(ctx);
    start_governance_watch(ctx);
    start_registry_heartbeat(ctx);
    start_secrets_refresh(ctx);
    start_schema_watch(ctx);
    if plan.reconcile_mcp {
        let mcp_orchestrator = create_mcp_orchestrator(ctx)?;
        reconcile_system_services(ctx, &mcp_orchestrator, events).await?;
    }
    if plan.reconcile_agents {
        run_agents_phase(ctx, events).await?;
    }
    let scheduler_handle = if plan.scheduler {
        run_scheduler_phase(ctx, events, scheduler_health).await?
    } else {
        None
    };
    if plan.runner_services {
        start_runner_services(ctx);
    }
    Ok(scheduler_handle)
}

async fn serve_until_shutdown(
    ctx: &AppContext,
    early: super::startup::EarlyServer,
    metrics_listener: Option<OwnedTask<()>>,
    scheduler_handle: Option<SchedulerHandle>,
    instance_claim: systemprompt_database::InstanceClaim,
) -> Result<()> {
    let timings = ctx.config().shutdown;
    tracing::info!(
        pre_stop_delay_secs = timings.pre_stop_delay_secs,
        drain_grace_secs = timings.drain_grace_secs,
        forced_exit_grace_secs = timings.forced_exit_grace_secs,
        total_budget_secs = timings.total_budget().as_secs(),
        "Shutdown budget; the orchestrator's termination grace must cover the total"
    );
    let restart = ctx.shutdown_request().clone();
    let readiness = Arc::clone(early.readiness());
    let serve_result = super::shutdown::join_within_drain_grace(
        early.join(),
        &restart,
        &timings,
        ctx.background_tasks().stream_drain(),
        &readiness,
    )
    .await;

    let forced_exit = super::shutdown::arm_forced_exit(restart, timings);
    if let Some(listener) = metrics_listener
        && listener.abort_and_join().await.is_some()
    {
        tracing::debug!("Metrics listener had already stopped on the shutdown signal");
    }
    super::shutdown::drain(ctx, scheduler_handle).await;
    instance_claim.release().await;
    forced_exit.abort();

    serve_result
}

fn start_runner_services(ctx: &AppContext) {
    let shared = Arc::new(ctx.clone());
    let mut started = 0usize;
    for extension in ctx.extension_registry().extensions() {
        for service in extension.runner_services() {
            let name = service.name();
            let actor = Actor::job(ctx.system_admin().id().clone(), name);
            let dependencies = Dependencies::new()
                .with(Arc::clone(ctx.app_paths_arc()))
                .with(Arc::clone(ctx.db_pool()))
                .with(Arc::clone(&shared));
            let job_ctx = JobContext::new(actor, dependencies);
            ctx.background_tasks()
                .spawn_cancellable(name, move |shutdown| async move {
                    service.run(job_ctx, shutdown).await;
                });
            tracing::info!(
                extension = extension.id(),
                service = name,
                "runner service started"
            );
            started += 1;
        }
    }
    tracing::info!(started, "runner services started");
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
    scheduler_health: &SchedulerHealth,
) -> Result<Option<SchedulerHandle>> {
    if let Some(tx) = events {
        tx.phase_started(Phase::Scheduler);
    }
    match initialize_scheduler(ctx, events, scheduler_health).await {
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

async fn start_metrics_listener(
    ctx: &AppContext,
    runs_scheduler: bool,
    readiness: &ReadinessState,
) -> Result<Option<OwnedTask<()>>> {
    let Some(port) = ctx.config().metrics_port else {
        return Ok(None);
    };
    let handle = super::metrics::install_recorder(
        &ctx.config().instance_id,
        &ctx.extension_registry().all_metrics(),
    )?;
    super::pool_metrics::spawn_sampler(ctx);
    super::ops_metrics::spawn_samplers(ctx, runs_scheduler);
    let addr = std::net::SocketAddr::new(ctx.config().host.parse()?, port);
    let state = super::metrics::MetricsState {
        handle,
        broadcasters: Arc::clone(ctx.event_broadcasters()),
    };
    Ok(Some(
        super::metrics::serve_metrics_listener(addr, state, readiness).await?,
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
