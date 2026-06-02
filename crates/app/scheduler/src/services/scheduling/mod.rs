//! Scheduler core — owns the [`tokio_cron_scheduler::JobScheduler`],
//! discovers inventory-registered jobs, and dispatches them under a typed
//! error boundary.

mod bootstrap;
mod dispatch;
mod lock;

use crate::error::{SchedulerError, SchedulerResult};
use crate::models::{JobConfig, SchedulerConfig};
use crate::repository::SchedulerRepository;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_logging::SystemSpan;
use systemprompt_models::auth::UserStatus;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job as JobTrait, JobContext};
use systemprompt_users::UserRepository;
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

    /// Resolves job owners, registers every enabled configured job against
    /// the cron scheduler, and starts dispatching on schedule. Returns the live
    /// [`SchedulerHandle`] so the caller can drain the dispatch loop on
    /// shutdown, or `None` when the scheduler is disabled in config.
    pub async fn start(self) -> SchedulerResult<Option<SchedulerHandle>> {
        if !self.config.enabled {
            info!("Scheduler is disabled");
            return Ok(None);
        }

        let resolved_owners = Self::resolve_owners(&self.db_pool, &self.config.jobs).await?;

        let registered_jobs = Self::discover_jobs();
        self.validate_configured_jobs(&registered_jobs)?;

        debug!(
            "Discovered {} jobs via inventory, {} configured",
            registered_jobs.len(),
            self.config.jobs.len()
        );

        let running_jobs: RunningJobs = Arc::new(Mutex::new(HashSet::new()));

        let scheduler = JobScheduler::new().await?;
        let ctx = RegistrationCtx {
            scheduler: &scheduler,
            registered_jobs: &registered_jobs,
            running_jobs: &running_jobs,
            owners: &resolved_owners,
        };
        self.register_jobs(&ctx).await?;
        scheduler.start().await?;

        info!("Scheduler started");
        Ok(Some(SchedulerHandle { scheduler }))
    }

    async fn resolve_owners(
        db_pool: &DbPool,
        jobs: &[JobConfig],
    ) -> SchedulerResult<HashMap<String, UserId>> {
        let users = UserRepository::new(db_pool)?;
        let mut resolved = HashMap::with_capacity(jobs.len());
        for job in jobs.iter().filter(|j| j.enabled) {
            let owner = users
                .find_by_name(job.owner.as_str())
                .await?
                .ok_or_else(|| SchedulerError::UnresolvedJobOwner {
                    job_name: job.name.clone(),
                    owner: job.owner.as_str().to_owned(),
                })?;
            if owner.status.as_deref() != Some(UserStatus::Active.as_str()) {
                return Err(SchedulerError::UnresolvedJobOwner {
                    job_name: job.name.clone(),
                    owner: job.owner.as_str().to_owned(),
                });
            }
            debug!(job_name = %job.name, owner = %owner.id, "resolved job owner");
            resolved.insert(job.name.clone(), owner.id);
        }
        Ok(resolved)
    }

    fn discover_jobs() -> HashMap<&'static str, &'static dyn JobTrait> {
        inventory::iter::<&'static dyn JobTrait>
            .into_iter()
            .map(|&job| (job.name(), job))
            .collect()
    }

    /// Fails loud if any name in `jobs` or `bootstrap_jobs` is absent from the
    /// inventory catalog. The inventory is authoritative — a configured name
    /// with no `submit_job!` registration is a wiring typo, not a silent skip.
    /// (A registered job absent from `jobs` is fine: an intentional cron
    /// opt-out.)
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
            return Err(SchedulerError::UnresolvedJobOwner {
                job_name: job_config.name.clone(),
                owner: job_config.owner.as_str().to_owned(),
            });
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

        let job =
            self.create_job_from_trait(&job_config.name, &schedule, ctx.running_jobs, actor)?;
        ctx.scheduler.add(job).await?;
        Ok(())
    }

    fn create_job_from_trait(
        &self,
        job_name: &str,
        schedule: &str,
        running_jobs: &RunningJobs,
        actor: Actor,
    ) -> SchedulerResult<Job> {
        let job_name_owned = job_name.to_owned();
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
