//! Scheduler core — owns the [`tokio_cron_scheduler::JobScheduler`],
//! discovers inventory-registered jobs, and dispatches them under a typed
//! error boundary.

mod dispatch;
mod lock;

use crate::error::{SchedulerError, SchedulerResult};
use crate::models::SchedulerConfig;
use crate::repository::SchedulerRepository;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_logging::SystemSpan;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job as JobTrait, JobContext};
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{Instrument, debug, info, warn};

pub(crate) type RunningJobs = Arc<Mutex<HashSet<String>>>;

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
    ) -> SchedulerResult<Self> {
        let repository = SchedulerRepository::new(&db_pool)?;
        Ok(Self {
            config,
            db_pool,
            repository,
            app_context,
        })
    }

    pub async fn start(self) -> SchedulerResult<()> {
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

        let running_jobs: RunningJobs = Arc::new(Mutex::new(HashSet::new()));

        let scheduler = JobScheduler::new().await?;
        self.register_jobs(&scheduler, &registered_jobs, &running_jobs)
            .await?;
        scheduler.start().await?;

        info!("Scheduler started");

        let startup_job_names = self.collect_startup_job_names(&registered_jobs);
        self.spawn_startup_jobs(startup_job_names, &running_jobs);

        Ok(())
    }

    fn collect_startup_job_names(
        &self,
        registered_jobs: &HashMap<&str, &'static dyn JobTrait>,
    ) -> Vec<String> {
        self.config
            .jobs
            .iter()
            .filter(|jc| jc.enabled)
            .filter_map(|jc| {
                registered_jobs
                    .get(jc.name.as_str())
                    .filter(|j| j.run_on_startup())
                    .map(|_| jc.name.clone())
            })
            .collect()
    }

    fn spawn_startup_jobs(&self, startup_job_names: Vec<String>, running_jobs: &RunningJobs) {
        if startup_job_names.is_empty() {
            return;
        }

        let count = startup_job_names.len();
        let db_pool = Arc::clone(&self.db_pool);
        let repository = self.repository.clone();
        let app_context = Arc::clone(&self.app_context);
        let running_jobs = Arc::clone(running_jobs);
        let distributed_lock = self.config.distributed_lock;

        info!(count, "Spawning startup jobs in background");

        tokio::spawn(async move {
            for job_name in startup_job_names {
                debug!(job_name = %job_name, "Running background startup job");
                dispatch::execute_job(dispatch::JobDispatch {
                    job_name,
                    db_pool: Arc::clone(&db_pool),
                    repository: repository.clone(),
                    app_context: Arc::clone(&app_context),
                    running_jobs: Arc::clone(&running_jobs),
                    distributed_lock,
                })
                .await;
            }
            info!("Background startup jobs completed");
        });
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
        running_jobs: &RunningJobs,
    ) -> SchedulerResult<()> {
        for job_config in &self.config.jobs {
            self.register_single_job(scheduler, registered_jobs, job_config, running_jobs)
                .await?;
        }
        Ok(())
    }

    async fn register_single_job(
        &self,
        scheduler: &JobScheduler,
        registered_jobs: &HashMap<&str, &'static dyn JobTrait>,
        job_config: &crate::models::JobConfig,
        running_jobs: &RunningJobs,
    ) -> SchedulerResult<()> {
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

        let job = self.create_job_from_trait(&job_config.name, &schedule, running_jobs)?;
        scheduler.add(job).await?;
        Ok(())
    }

    fn create_job_from_trait(
        &self,
        job_name: &str,
        schedule: &str,
        running_jobs: &RunningJobs,
    ) -> SchedulerResult<Job> {
        let job_name_owned = job_name.to_string();
        let schedule_owned = schedule.to_string();
        let db_pool = Arc::clone(&self.db_pool);
        let repository = self.repository.clone();
        let app_context = Arc::clone(&self.app_context);
        let running_jobs = Arc::clone(running_jobs);
        let distributed_lock = self.config.distributed_lock;

        let job = Job::new_async(schedule_owned.as_str(), move |_uuid, _lock| {
            let job_name = job_name_owned.clone();
            let db_pool = Arc::clone(&db_pool);
            let repository = repository.clone();
            let app_context = Arc::clone(&app_context);
            let running_jobs = Arc::clone(&running_jobs);

            Box::pin(async move {
                let span = SystemSpan::new(&format!("scheduler:{job_name}"));
                dispatch::execute_job(dispatch::JobDispatch {
                    job_name,
                    db_pool,
                    repository,
                    app_context,
                    running_jobs,
                    distributed_lock,
                })
                .instrument(span.span().clone())
                .await;
            })
        })
        .map_err(SchedulerError::from)?;

        Ok(job)
    }
}

pub(crate) fn make_job_context(db_pool: DbPool, app_context: Arc<AppContext>) -> JobContext {
    let app_paths_any: Arc<dyn std::any::Any + Send + Sync> =
        Arc::new(Arc::clone(app_context.app_paths_arc()));
    let db_pool_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(db_pool);
    let app_context_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(app_context);
    JobContext::new(db_pool_any, app_context_any, app_paths_any)
}
