//! Scheduler core — owns the [`tokio_cron_scheduler::JobScheduler`],
//! discovers inventory-registered jobs, and dispatches them under a typed
//! error boundary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bootstrap;
mod dispatch;
mod lock;
mod owners;

use crate::error::{SchedulerError, SchedulerResult};
use crate::models::{JobConfig, SchedulerConfig, SkippedJob};
use crate::repository::SchedulerRepository;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_logging::SystemSpan;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job as JobTrait, JobContext};
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{Instrument, debug, info, warn};

pub(crate) type RunningJobs = Arc<Mutex<HashSet<String>>>;

/// Live handle to a started scheduler, returned by [`SchedulerService::start`].
///
/// Holding it keeps the cron dispatch loop owned by the caller so it can be
/// drained on shutdown; dropping it without [`SchedulerHandle::shutdown`]
/// leaves the loop running until process exit.
pub struct SchedulerHandle {
    scheduler: JobScheduler,
}

impl std::fmt::Debug for SchedulerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchedulerHandle").finish_non_exhaustive()
    }
}

impl SchedulerHandle {
    pub async fn shutdown(mut self) -> SchedulerResult<()> {
        self.scheduler
            .shutdown()
            .await
            .map_err(SchedulerError::from)
    }
}

/// Outcome of [`SchedulerService::start`].
///
/// `handle` is `None` when the scheduler is disabled; `degraded` lists jobs
/// dropped because their explicit owner did not resolve, for the caller to
/// surface as a health signal.
#[derive(Debug)]
pub struct SchedulerStartup {
    pub handle: Option<SchedulerHandle>,
    pub degraded: Vec<SkippedJob>,
}

impl SchedulerStartup {
    const fn disabled() -> Self {
        Self {
            handle: None,
            degraded: Vec::new(),
        }
    }
}

struct RegistrationCtx<'a> {
    scheduler: &'a JobScheduler,
    registered_jobs: &'a HashMap<&'a str, &'static dyn JobTrait>,
    running_jobs: &'a RunningJobs,
    owners: &'a HashMap<String, UserId>,
}

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

    /// A job whose explicit `owner` does not resolve to an active user is
    /// skipped rather than aborting the scheduler; the skipped set rides back
    /// in [`SchedulerStartup::degraded`] for the health signal.
    pub async fn start(self) -> SchedulerResult<SchedulerStartup> {
        if !self.config.enabled {
            info!("Scheduler is disabled");
            return Ok(SchedulerStartup::disabled());
        }

        let resolved = self.resolve_owners(true).await?;

        let registered_jobs = Self::discover_jobs();
        self.validate_configured_jobs(&registered_jobs)?;

        debug!(
            "Discovered {} jobs via inventory, {} configured",
            registered_jobs.len(),
            self.config.jobs.len()
        );
        self.warn_unscheduled_jobs(&registered_jobs);

        let running_jobs: RunningJobs = Arc::new(Mutex::new(HashSet::new()));

        let scheduler = JobScheduler::new().await?;
        let ctx = RegistrationCtx {
            scheduler: &scheduler,
            registered_jobs: &registered_jobs,
            running_jobs: &running_jobs,
            owners: resolved.owner_map(),
        };
        self.register_jobs(&ctx).await?;
        scheduler.start().await?;

        info!("Scheduler started");
        Ok(SchedulerStartup {
            handle: Some(SchedulerHandle { scheduler }),
            degraded: resolved.into_degraded(),
        })
    }

    fn discover_jobs() -> HashMap<&'static str, &'static dyn JobTrait> {
        inventory::iter::<&'static dyn JobTrait>
            .into_iter()
            .map(|&job| (job.name(), job))
            .collect()
    }

    fn validate_configured_jobs(
        &self,
        registered_jobs: &HashMap<&'static str, &'static dyn JobTrait>,
    ) -> SchedulerResult<()> {
        let mut unknown: Vec<&str> = Vec::new();
        for name in self
            .config
            .jobs
            .iter()
            .map(|job| job.name.as_str())
            .chain(self.config.bootstrap_jobs.iter().map(String::as_str))
        {
            if !registered_jobs.contains_key(name) && !unknown.contains(&name) {
                unknown.push(name);
            }
        }
        if unknown.is_empty() {
            Ok(())
        } else {
            Err(SchedulerError::UnknownJob {
                names: unknown.join(", "),
            })
        }
    }

    /// An inventory job absent from `scheduler.jobs` is silently never
    /// scheduled; surface each one at boot so a dead pipeline (e.g. bot
    /// classification jobs missing from a deployed profile) is visible.
    fn warn_unscheduled_jobs(
        &self,
        registered_jobs: &HashMap<&'static str, &'static dyn JobTrait>,
    ) {
        let configured: HashSet<&str> = self
            .config
            .jobs
            .iter()
            .map(|job| job.name.as_str())
            .chain(self.config.bootstrap_jobs.iter().map(String::as_str))
            .collect();
        for name in registered_jobs.keys() {
            if !configured.contains(name) {
                warn!(
                    job = %name,
                    "job is available in this build but has no scheduler.jobs entry; it will never run"
                );
            }
        }
    }

    async fn register_jobs(&self, ctx: &RegistrationCtx<'_>) -> SchedulerResult<()> {
        for job_config in &self.config.jobs {
            self.register_single_job(ctx, job_config).await?;
        }
        Ok(())
    }

    async fn register_single_job(
        &self,
        ctx: &RegistrationCtx<'_>,
        job_config: &JobConfig,
    ) -> SchedulerResult<()> {
        if !job_config.enabled {
            debug!("Skipping disabled job: {}", job_config.name);
            return Ok(());
        }

        let Some(registered_job) = ctx.registered_jobs.get(job_config.name.as_str()) else {
            warn!("Job '{}' not found in inventory, skipping", job_config.name);
            return Ok(());
        };

        let Some(owner_id) = ctx.owners.get(&job_config.name).cloned() else {
            warn!(job = %job_config.name, "no resolved owner for job, skipping");
            return Ok(());
        };
        let actor = Actor::job(owner_id, job_config.name.clone());

        let schedule = job_config
            .schedule
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| registered_job.schedule().to_owned());

        if schedule.is_empty() {
            info!(
                job = %job_config.name,
                "Job has an empty schedule; bootstrap/manual-only, not cron-scheduled"
            );
            return Ok(());
        }

        self.repository
            .upsert_job(&job_config.name, &schedule, job_config.enabled)
            .await?;

        let job = self.create_job_from_trait(job_config, &schedule, ctx.running_jobs, actor)?;
        ctx.scheduler.add(job).await?;
        Ok(())
    }

    fn create_job_from_trait(
        &self,
        job_config: &JobConfig,
        schedule: &str,
        running_jobs: &RunningJobs,
        actor: Actor,
    ) -> SchedulerResult<Job> {
        let enforce = job_config.enforce;
        let job_name_owned = job_config.name.clone();
        let schedule_owned = schedule.to_owned();
        let db_pool = Arc::clone(&self.db_pool);
        let repository = self.repository.clone();
        let app_context = Arc::clone(&self.app_context);
        let running_jobs = Arc::clone(running_jobs);
        let distributed_lock = self.config.distributed_lock;

        let job = Job::new_async(schedule_owned.as_str(), move |_uuid, _lock| {
            let job_name = job_name_owned.clone();
            let actor = actor.clone();
            let db_pool = Arc::clone(&db_pool);
            let repository = repository.clone();
            let app_context = Arc::clone(&app_context);
            let running_jobs = Arc::clone(&running_jobs);

            Box::pin(async move {
                let span = SystemSpan::new(&format!("scheduler:{job_name}"));
                dispatch::execute_job(dispatch::JobDispatch {
                    job_name,
                    actor,
                    db_pool,
                    repository,
                    app_context,
                    running_jobs,
                    distributed_lock,
                    enforce,
                })
                .instrument(span.span().clone())
                .await;
            })
        })
        .map_err(SchedulerError::from)?;

        Ok(job)
    }
}

pub(crate) fn make_job_context(
    actor: Actor,
    db_pool: DbPool,
    app_context: Arc<AppContext>,
) -> JobContext {
    let app_paths_any: Arc<dyn std::any::Any + Send + Sync> =
        Arc::new(Arc::clone(app_context.app_paths_arc()));
    let db_pool_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(db_pool);
    let app_context_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(app_context);
    JobContext::new(actor, db_pool_any, app_context_any, app_paths_any)
}
