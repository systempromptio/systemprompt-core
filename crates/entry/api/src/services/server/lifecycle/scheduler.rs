//! Scheduler start/stop wiring in the server lifecycle.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use std::sync::Arc;
use systemprompt_loader::ConfigLoader;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::SchedulerConfig;
use systemprompt_scheduler::services::{SchedulerHandle, SchedulerService};
use systemprompt_traits::{OptionalStartupEventExt, StartupEventSender};

use crate::services::server::scheduler_health;

pub(in crate::services::server) async fn initialize_scheduler(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<Option<SchedulerHandle>> {
    events.scheduler_initializing();

    let config = ConfigLoader::load().map_or_else(
        |e| {
            tracing::warn!(error = %e, "Failed to load scheduler config, using defaults");
            SchedulerConfig::with_system_admin()
        },
        |c| {
            c.scheduler
                .unwrap_or_else(SchedulerConfig::with_system_admin)
        },
    );

    let scheduler =
        SchedulerService::new(config, Arc::clone(ctx.db_pool()), Arc::new(ctx.clone()))?;

    let job_count = scheduler.run_bootstrap_jobs(events).await?;
    let startup = scheduler.start().await?;

    if !startup.degraded.is_empty() {
        tracing::error!(
            count = startup.degraded.len(),
            "scheduler started degraded: jobs skipped due to unresolved owners"
        );
    }
    scheduler_health::record(startup.degraded);

    events.scheduler_ready(job_count);
    Ok(startup.handle)
}
