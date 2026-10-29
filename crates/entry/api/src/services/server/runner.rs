//! Server run loop: MCP orchestrator wiring and lifecycle supervision.
//!
//! Phases follow `server.role` ([`super::routes::role::lifecycle_plan`]): a
//! gateway or api node runs no MCP servers, agents or scheduler; an `all` node
//! supervises them all itself; a worker supervises the processes whose
//! per-process leases it holds ([`super::lifecycle::process_ownership`]); a
//! runner node starts each extension's [`systemprompt_traits::RunnerService`]
//! under a supervisor that restarts it with backoff when it stops.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_identifiers::Actor;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::services::SchedulerHandle;
use systemprompt_traits::{
    Dependencies, JobContext, OwnedTask, Phase, RunnerService, StartupEventExt, StartupEventSender,
};

use super::lifecycle::{
    reconcile_system_services, start_event_bridge, start_governance_watch,
    start_leased_supervision, start_registry_heartbeat, start_schema_watch, start_secrets_refresh,
    start_services_watch,
};
use super::phases::{run_agents_phase, run_scheduler_phase};
use super::readiness::ReadinessState;
use super::routes::role::{LifecyclePlan, ProcessOwnership};
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
    tracing::info!(
        role = %ctx.config().role,
        scheduler = plan.scheduler,
        process_ownership = ?plan.process_ownership,
        runner_services = plan.runner_services,
        "node role"
    );

    let scheduler_handle =
        start_role_phases(&ctx, plan, events.as_ref(), early.scheduler_health()).await?;
    super::boot_steps::reconcile_document_folders(&ctx).await?;

    if let Some(ref tx) = events {
        tx.phase_started(Phase::ApiServer);
    }
    let router =
        crate::services::server::setup_api_server(&ctx, events.as_ref(), early.readiness())?
            .layer(axum::Extension(Arc::clone(early.scheduler_health())));
    super::boot_steps::start_accounting_recovery(&ctx).await?;
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

    systemprompt_logging::OutputState::current().set_startup_mode(false);

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
    start_services_watch(ctx);
    start_registry_heartbeat(ctx);
    start_secrets_refresh(ctx);
    start_schema_watch(ctx);
    match plan.process_ownership {
        ProcessOwnership::None => {},
        ProcessOwnership::Local => {
            if plan.reconcile_mcp {
                let mcp_orchestrator = create_mcp_orchestrator(ctx)?;
                reconcile_system_services(ctx, &mcp_orchestrator, events).await?;
            }
            if plan.reconcile_agents {
                run_agents_phase(ctx, events).await?;
            }
        },
        ProcessOwnership::Leased => start_leased_supervision(ctx),
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
    #[cfg(feature = "notifications")]
    {
        let consumer: Arc<dyn RunnerService> = ctx.notifications().consumer.clone();
        spawn_runner_service(ctx, &shared, consumer);
        tracing::info!(service = "notifications", "runner service started");
        started += 1;
    }
    for extension in ctx.extension_registry().extensions() {
        for service in extension.runner_services() {
            let name = service.name();
            spawn_runner_service(ctx, &shared, service);
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

fn spawn_runner_service(
    ctx: &AppContext,
    shared: &Arc<AppContext>,
    service: Arc<dyn RunnerService>,
) {
    let name = service.name();
    let admin = ctx.system_admin().id().clone();
    let shared = Arc::clone(shared);
    super::runner_supervisor::supervise(ctx.background_tasks(), service, move || {
        let dependencies = Dependencies::new()
            .with(Arc::clone(shared.app_paths_arc()))
            .with(Arc::clone(shared.db_pool()))
            .with::<Arc<dyn systemprompt_extension::ExtensionContext>>(Arc::<AppContext>::clone(
                &shared,
            ))
            .with(Arc::clone(&shared));
        JobContext::new(Actor::job(admin.clone(), name), dependencies)
    });
}

pub(in crate::services::server) fn create_mcp_orchestrator(
    ctx: &AppContext,
) -> Result<Arc<systemprompt_mcp::services::McpOrchestrator>> {
    use systemprompt_mcp::services::McpOrchestrator;
    let manager = McpOrchestrator::new(
        (**ctx.service_repository()).clone(),
        Arc::clone(ctx.app_paths_arc()),
        ctx.mcp_registry().clone(),
        Arc::clone(ctx.spawner()),
        Arc::clone(ctx.secrets()),
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
