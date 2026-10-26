//! Scheduler start/stop wiring in the server lifecycle.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::SchedulerConfig;
use systemprompt_scheduler::services::{
    ConfigReconcileService, RECONCILE_LOCK_WAIT, ReconcileTrigger, SchedulerHandle,
    SchedulerService,
};
use systemprompt_traits::{StartupEventExt, StartupEventSender};

use crate::services::server::scheduler_health::SchedulerHealth;

pub(in crate::services::server) async fn initialize_scheduler(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
    scheduler_health: &SchedulerHealth,
) -> Result<Option<SchedulerHandle>> {
    events.scheduler_initializing();
    reconcile_config_planes(ctx).await?;

    let config = ctx
        .services_config()
        .scheduler
        .clone()
        .unwrap_or_else(SchedulerConfig::with_system_admin);

    let scheduler = SchedulerService::new(config, Arc::clone(ctx.db_pool()), Arc::new(ctx.clone()));

    let available_jobs = scheduler.run_bootstrap_jobs(events).await?;
    let startup = scheduler.start().await?;

    if !startup.degraded.is_empty() {
        tracing::error!(
            count = startup.degraded.len(),
            "scheduler started degraded: jobs skipped due to unresolved owners"
        );
    }
    scheduler_health.record(startup.degraded);

    events.scheduler_ready(startup.scheduled, available_jobs);
    Ok(startup.handle)
}

async fn reconcile_config_planes(ctx: &AppContext) -> Result<()> {
    let service = ConfigReconcileService::from_app_context(
        &Arc::new(ctx.clone()),
        (*crate::repository::config_plane_state(ctx.db_pool())).clone(),
    )?;
    let report = service
        .reconcile_exclusive(ReconcileTrigger::Boot, RECONCILE_LOCK_WAIT)
        .await?;
    let failures: Vec<String> = report
        .failures()
        .iter()
        .map(|p| format!("{}: {}", p.plane, p.error.as_deref().unwrap_or("failed")))
        .collect();
    if !failures.is_empty() {
        bail!(
            "config planes failed to reconcile at boot: {}",
            failures.join("; ")
        );
    }
    tracing::info!(
        planes = report.planes.len(),
        drift_rows = report.drift_rows(),
        "config planes checked at boot; nothing applied"
    );
    Ok(())
}
