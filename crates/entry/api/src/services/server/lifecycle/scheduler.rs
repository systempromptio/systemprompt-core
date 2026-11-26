//! Scheduler start/stop wiring in the server lifecycle.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_extension::AdmittedSchedule;
use systemprompt_extension::prelude::ChangeActor;
use systemprompt_loader::ServicesState;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::SchedulerConfig;
use systemprompt_scheduler::services::{
    ConfigApplyService, ConfigReconcileService, RECONCILE_LOCK_WAIT, ReconcileTrigger,
    SchedulerHandle, SchedulerService,
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

    let state = ctx.kernel().state();
    let db = state.get::<DbPool>()?;
    let schedule = Arc::new(ctx.kernel().resolved().get::<AdmittedSchedule>()?);
    let config = state
        .get::<ServicesState>()?
        .config()
        .scheduler
        .clone()
        .unwrap_or_else(|| {
            SchedulerConfig::with_system_admin_admitted(|name| schedule.contains_job(name))
        });
    let scheduler = SchedulerService::new(config, db, Arc::new(ctx.clone()), schedule)?;

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
        (*crate::repository::config_plane_state(ctx.kernel().state().get::<DbPool>()?)).clone(),
    )?;
    let applier = ConfigApplyService::new(service);
    let file_planes = applier
        .apply_file_managed(
            ChangeActor::Boot,
            "file-authority planes applied at boot",
            RECONCILE_LOCK_WAIT,
        )
        .await?;
    if !file_planes.planes.is_empty() {
        tracing::info!(
            planes = file_planes.planes.len(),
            written = file_planes.applied(),
            "file-authority config planes applied at boot"
        );
    }
    let report = applier
        .reconcile()
        .reconcile_exclusive(ReconcileTrigger::Boot, RECONCILE_LOCK_WAIT)
        .await?;
    let failures: Vec<String> = report
        .failures()
        .iter()
        .map(|p| {
            format!(
                "{}: {}",
                p.plane,
                p.failure
                    .as_ref()
                    .map_or_else(|| "failed".to_owned(), ToString::to_string)
            )
        })
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
        "config planes checked at boot"
    );
    Ok(())
}
