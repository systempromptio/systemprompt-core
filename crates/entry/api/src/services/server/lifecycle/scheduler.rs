use anyhow::Result;
use std::sync::Arc;
use systemprompt_loader::ConfigLoader;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::SchedulerConfig;
use systemprompt_scheduler::services::SchedulerService;
use systemprompt_traits::{OptionalStartupEventExt, StartupEventSender};

pub async fn initialize_scheduler(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    events.scheduler_initializing();

    let admin = ctx.system_admin();
    let config = ConfigLoader::load().map_or_else(
        |e| {
            tracing::warn!(error = %e, "Failed to load scheduler config, using defaults");
            SchedulerConfig::with_system_admin(admin)
        },
        |c| {
            c.scheduler
                .unwrap_or_else(|| SchedulerConfig::with_system_admin(admin))
        },
    );

    let scheduler =
        SchedulerService::new(config, Arc::clone(ctx.db_pool()), Arc::new(ctx.clone()))?;

    let job_count = scheduler.run_bootstrap_jobs(events).await?;
    scheduler.start().await?;

    events.scheduler_ready(job_count);
    Ok(())
}
