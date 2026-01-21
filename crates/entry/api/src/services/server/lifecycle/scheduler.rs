use anyhow::Result;
use std::sync::Arc;
use systemprompt_loader::ConfigLoader;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::models::JobStatus;
use systemprompt_scheduler::repository::SchedulerRepository;
use systemprompt_scheduler::services::SchedulerService;
use systemprompt_scheduler::SchedulerConfig;
use systemprompt_traits::{Job, JobContext, StartupEvent, StartupEventSender};

pub async fn initialize_scheduler(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    if let Some(tx) = events {
        if tx.unbounded_send(StartupEvent::SchedulerInitializing).is_err() {
            tracing::debug!("Startup event receiver dropped");
        }
    }

    let scheduler_config = ConfigLoader::load()
        .map(|config| {
            config.scheduler.unwrap_or_else(|| {
                tracing::info!("No scheduler config found, using defaults");
                SchedulerConfig::default()
            })
        })
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to load scheduler config, using defaults");
            SchedulerConfig::default()
        });

    let bootstrap_jobs = scheduler_config.bootstrap_jobs.clone();

    let scheduler = SchedulerService::new(
        scheduler_config,
        ctx.db_pool().clone(),
        Arc::new(ctx.clone()),
    )?;

    scheduler.start().await?;

    let db_pool = ctx.db_pool().clone();
    let scheduler_repo = SchedulerRepository::new(&db_pool)?;

    let db_pool_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(db_pool.clone());
    let app_context_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(Arc::new(ctx.clone()));
    let job_ctx = JobContext::new(db_pool_any, app_context_any);

    for job_name in &bootstrap_jobs {
        let job = inventory::iter::<&'static dyn Job>
            .into_iter()
            .find(|&j| j.name() == job_name)
            .copied();

        if let Some(job) = job {
            run_bootstrap_job(&scheduler_repo, job, &job_ctx, events).await;
        } else {
            tracing::warn!(job = %job_name, "Bootstrap job not found in registry");
        }
    }

    if let Some(tx) = events {
        let job_count = inventory::iter::<&'static dyn Job>.into_iter().count();
        if tx.unbounded_send(StartupEvent::SchedulerReady { job_count }).is_err() {
            tracing::debug!("Startup event receiver dropped");
        }
    }

    Ok(())
}

async fn run_bootstrap_job(
    scheduler_repo: &SchedulerRepository,
    job: &dyn Job,
    ctx: &JobContext,
    events: Option<&StartupEventSender>,
) {
    let job_name = job.name();

    if let Some(tx) = events {
        if tx
            .unbounded_send(StartupEvent::BootstrapJobStarted {
                name: job_name.to_string(),
            })
            .is_err()
        {
            tracing::debug!("Startup event receiver dropped");
        }
    }

    if let Err(e) = scheduler_repo.increment_run_count(job_name).await {
        tracing::warn!(error = %e, job = %job_name, "Failed to increment job run count");
    }

    match job.execute(ctx).await {
        Ok(result) if result.success => {
            if let Some(tx) = events {
                if tx
                    .unbounded_send(StartupEvent::BootstrapJobCompleted {
                        name: job_name.to_string(),
                        success: true,
                        message: None,
                    })
                    .is_err()
                {
                    tracing::debug!("Startup event receiver dropped");
                }
            }
            if let Err(e) = scheduler_repo
                .update_job_execution(job_name, JobStatus::Success, None, None)
                .await
            {
                tracing::warn!(error = %e, job = %job_name, "Failed to update job execution status");
            }
        },
        Ok(result) => {
            let msg = result
                .message
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string());
            if let Some(tx) = events {
                if tx
                    .unbounded_send(StartupEvent::BootstrapJobCompleted {
                        name: job_name.to_string(),
                        success: false,
                        message: Some(msg),
                    })
                    .is_err()
                {
                    tracing::debug!("Startup event receiver dropped");
                }
            }
            if let Err(e) = scheduler_repo
                .update_job_execution(job_name, JobStatus::Failed, result.message.as_deref(), None)
                .await
            {
                tracing::warn!(error = %e, job = %job_name, "Failed to update job execution status");
            }
        },
        Err(e) => {
            if let Some(tx) = events {
                if tx
                    .unbounded_send(StartupEvent::BootstrapJobCompleted {
                        name: job_name.to_string(),
                        success: false,
                        message: Some(e.to_string()),
                    })
                    .is_err()
                {
                    tracing::debug!("Startup event receiver dropped");
                }
            }
            if let Err(update_err) = scheduler_repo
                .update_job_execution(job_name, JobStatus::Failed, Some(&e.to_string()), None)
                .await
            {
                tracing::warn!(error = %update_err, job = %job_name, "Failed to update job execution status");
            }
        },
    }
}
