use crate::models::{JobStatus, SchedulerConfig};
use crate::repository::SchedulerRepository;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_core_logging::SystemSpan;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job as JobTrait, JobContext};
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error, info, warn, Instrument};

#[derive(Debug)]
pub struct SchedulerService {
    config: SchedulerConfig,
    db_pool: DbPool,
    repository: SchedulerRepository,
    app_context: Arc<AppContext>,
}

impl SchedulerService {
    pub fn new(
        config: SchedulerConfig,
        db_pool: DbPool,
        app_context: Arc<AppContext>,
    ) -> Result<Self> {
        let repository = SchedulerRepository::new(&db_pool)?;
        Ok(Self {
            config,
            db_pool,
            repository,
            app_context,
        })
    }

    #[allow(clippy::cognitive_complexity)]
    pub async fn start(self) -> Result<()> {
        if !self.config.enabled {
            info!("Scheduler is disabled");
            return Ok(());
        }

        let registered_jobs = Self::discover_jobs();

        debug!(
            "Discovered {} jobs via inventory, {} configured",
            registered_jobs.len(),
            self.config.jobs.len()
        );

        let scheduler = JobScheduler::new().await?;
        self.register_jobs(&scheduler, &registered_jobs).await?;
        scheduler.start().await?;

        info!("Scheduler started");

        Ok(())
    }

    fn discover_jobs() -> HashMap<&'static str, &'static dyn JobTrait> {
        inventory::iter::<&'static dyn JobTrait>
            .into_iter()
            .map(|&job| (job.name(), job))
            .collect()
    }

    async fn register_jobs(
        &self,
        scheduler: &JobScheduler,
        registered_jobs: &HashMap<&str, &'static dyn JobTrait>,
    ) -> Result<()> {
        for job_config in &self.config.jobs {
            self.register_single_job(scheduler, registered_jobs, job_config)
                .await?;
        }
        Ok(())
    }

    #[allow(clippy::cognitive_complexity)]
    async fn register_single_job(
        &self,
        scheduler: &JobScheduler,
        registered_jobs: &HashMap<&str, &'static dyn JobTrait>,
        job_config: &crate::models::JobConfig,
    ) -> Result<()> {
        if !job_config.enabled {
            debug!("Skipping disabled job: {}", job_config.name);
            return Ok(());
        }

        let Some(registered_job) = registered_jobs.get(job_config.name.as_str()) else {
            warn!("Job '{}' not found in inventory, skipping", job_config.name);
            return Ok(());
        };

        let schedule = job_config
            .schedule
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| registered_job.schedule().to_string());

        self.repository
            .upsert_job(&job_config.name, &schedule, job_config.enabled)
            .await?;

        let job = self.create_job_from_trait(&job_config.name, &schedule)?;
        scheduler.add(job).await?;
        Ok(())
    }

    fn create_job_from_trait(&self, job_name: &str, schedule: &str) -> Result<Job> {
        let job_name_owned = job_name.to_string();
        let schedule_owned = schedule.to_string();
        let db_pool = Arc::clone(&self.db_pool);
        let repository = self.repository.clone();
        let app_context = Arc::clone(&self.app_context);

        let job = Job::new_async(schedule_owned.as_str(), move |_uuid, _lock| {
            let job_name = job_name_owned.clone();
            let db_pool = Arc::clone(&db_pool);
            let repository = repository.clone();
            let app_context = Arc::clone(&app_context);

            Box::pin(async move {
                let span = SystemSpan::new(&format!("scheduler:{job_name}"));
                Self::execute_job(job_name, db_pool, repository, app_context)
                    .instrument(span.span().clone())
                    .await;
            })
        })?;

        Ok(job)
    }

    async fn execute_job(
        job_name: String,
        db_pool: DbPool,
        repository: SchedulerRepository,
        app_context: Arc<AppContext>,
    ) {
        debug!(job_name = %job_name, "Starting job");

        repository
            .increment_run_count(&job_name)
            .await
            .map_err(|e| error!(job_name = %job_name, error = %e, "Failed to increment run count"))
            .ok();

        let result = Self::find_and_execute_job(&job_name, db_pool, app_context).await;
        Self::handle_job_result(&job_name, result, &repository).await;
    }

    fn find_job(job_name: &str) -> Option<&'static dyn JobTrait> {
        inventory::iter::<&'static dyn JobTrait>
            .into_iter()
            .find(|&j| j.name() == job_name)
            .copied()
    }

    async fn find_and_execute_job(
        job_name: &str,
        db_pool: DbPool,
        app_context: Arc<AppContext>,
    ) -> Result<systemprompt_traits::JobResult> {
        let job = Self::find_job(job_name).ok_or_else(|| {
            error!(job_name = %job_name, "Job not found in inventory");
            anyhow::anyhow!("Job not found: {}", job_name)
        })?;

        // db_pool is Arc<Database>, we wrap in Arc to store Arc<Database> as the
        // type-erased value app_context is Arc<AppContext>, we wrap in Arc to
        // store Arc<AppContext> as the type-erased value Jobs will access these
        // via ctx.db_pool::<DbPool>() and ctx.app_context::<Arc<AppContext>>()
        let db_pool_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(db_pool);
        let app_context_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(app_context);
        let ctx = JobContext::new(db_pool_any, app_context_any);
        job.execute(&ctx).await
    }

    #[allow(clippy::cognitive_complexity)]
    async fn handle_job_result(
        job_name: &str,
        result: Result<systemprompt_traits::JobResult>,
        repository: &SchedulerRepository,
    ) {
        match result {
            Ok(job_result) if job_result.success => {
                Self::record_success(job_name, &job_result, repository).await;
            },
            Ok(job_result) => {
                Self::record_failure(job_name, job_result.message.as_deref(), repository).await;
                error!(job_name = %job_name, message = ?job_result.message, "Job failed");
            },
            Err(e) => {
                let error_msg = e.to_string();
                error!(error = %error_msg, "Job failed with error");
                Self::record_failure(job_name, Some(&error_msg), repository).await;
            },
        }
    }

    async fn record_success(
        job_name: &str,
        job_result: &systemprompt_traits::JobResult,
        repository: &SchedulerRepository,
    ) {
        repository
            .update_job_execution(job_name, JobStatus::Success, None, None)
            .await
            .map_err(|e| error!(job_name = %job_name, error = %e, "Failed to update job execution status"))
            .ok();

        debug!(
            job_name = %job_name,
            duration_ms = job_result.duration_ms,
            "Job completed"
        );
    }

    async fn record_failure(
        job_name: &str,
        message: Option<&str>,
        repository: &SchedulerRepository,
    ) {
        repository
            .update_job_execution(job_name, JobStatus::Failed, message, None)
            .await
            .map_err(
                |e| error!(job_name = %job_name, error = %e, "Failed to update failed job status"),
            )
            .ok();
    }
}
