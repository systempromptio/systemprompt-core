//! Scheduler core — owns the [`tokio_cron_scheduler::JobScheduler`],
//! schedules the jobs of the admitted composition ([`AdmittedSchedule`]),
//! and dispatches them under a typed error boundary.
//!
//! Start never deletes a `scheduled_jobs` row. A node schedules the jobs its
//! admitted members contribute and ignores every other row, so nodes of
//! different releases or compositions sharing one database keep each other's
//! rows, operator `enabled` flags and run history. A row no node registers any
//! longer is removed only by `infra jobs prune`.
//! Jobs the build registers but the configuration never names are handled by
//! `scheduler.unlisted_jobs`.
//!
//! Before the cron loop starts, every job that would be scheduled has its
//! [`systemprompt_traits::Job::requires`] checked against the context it will
//! run with; an unmet requirement refuses the start with
//! [`SchedulerError::UnmetRequirement`]. A job whose `configured()` is false
//! is not scheduled and not checked. Once the loop runs, each job's missed
//! instants are applied on a background task.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bootstrap;
mod claim;
mod dispatch;
mod instants;
mod job_context;
mod missed;
mod owners;
mod registration;
mod unlisted;

use crate::error::{SchedulerError, SchedulerResult};
use crate::models::{SchedulerConfig, SkippedJob};
use crate::repository::SchedulerRepository;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::AdmittedSchedule;
use systemprompt_identifiers::{InstanceId, JobName, UserId};
use systemprompt_loader::BootContext;
use systemprompt_logging::LoggingRuntime;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_runtime::AppContext;
use systemprompt_traits::Job as JobTrait;
use systemprompt_traits::{BackgroundTasks, Dependencies};
use tokio::sync::Mutex;
use tokio_cron_scheduler::JobScheduler;
use tracing::{debug, info, warn};

pub(crate) use claim::{ClaimPolicy, claim_policy};
pub use instants::{CronInstants, DynSchedulerClock, SchedulerClock, SystemClock};
pub(crate) use job_context::{
    JobDeployment, JobRun, deployment_dependencies, job_dependencies, kernel_handles,
    make_job_context,
};
pub use unlisted::unlisted_job_names;

pub(crate) type RunningJobs = Arc<Mutex<HashSet<JobName>>>;

pub(crate) type JobMap = HashMap<&'static str, Arc<dyn JobTrait>>;

pub(crate) fn job_map(schedule: &AdmittedSchedule) -> JobMap {
    schedule
        .jobs()
        .iter()
        .map(|(_, job)| (job.name(), Arc::clone(job)))
        .collect()
}

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
/// dropped because their explicit owner did not resolve or because
/// registration failed, for the caller to surface as a health signal.
/// `scheduled` is how many jobs are actually on the cron loop, as distinct
/// from the number of jobs the admitted composition contributes.
#[derive(Debug)]
pub struct SchedulerStartup {
    pub handle: Option<SchedulerHandle>,
    pub degraded: Vec<SkippedJob>,
    pub scheduled: usize,
}

impl SchedulerStartup {
    const fn disabled() -> Self {
        Self {
            handle: None,
            degraded: Vec::new(),
            scheduled: 0,
        }
    }
}

struct RegistrationCtx<'a> {
    scheduler: &'a JobScheduler,
    registered_jobs: &'a JobMap,
    running_jobs: &'a RunningJobs,
    owners: &'a HashMap<JobName, UserId>,
}

#[must_use]
pub fn unknown_job_names(config: &SchedulerConfig, schedule: &AdmittedSchedule) -> Vec<JobName> {
    collect_unknown_job_names(config, |name| schedule.contains_job(name.as_str()))
        .into_iter()
        .cloned()
        .collect()
}

fn collect_unknown_job_names(
    config: &SchedulerConfig,
    is_registered: impl Fn(&JobName) -> bool,
) -> Vec<&JobName> {
    let mut unknown: Vec<&JobName> = Vec::new();
    for name in config
        .jobs
        .iter()
        .map(|job| &job.name)
        .chain(config.bootstrap_jobs.iter())
    {
        if !is_registered(name) && !unknown.contains(&name) {
            unknown.push(name);
        }
    }
    unknown
}

pub struct SchedulerService {
    config: SchedulerConfig,
    repository: SchedulerRepository,
    users: Option<systemprompt_traits::DynUserProvider>,
    logging_repository: systemprompt_logging::LoggingRepository,
    app_context: Arc<AppContext>,
    db_pool: DbPool,
    instance_id: InstanceId,
    system_admin_id: UserId,
    log_instance_id: Option<InstanceId>,
    deployment: Dependencies,
    background_tasks: BackgroundTasks,
    schedule: Arc<AdmittedSchedule>,
    clock: DynSchedulerClock,
}

impl std::fmt::Debug for SchedulerService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchedulerService")
            .field("config", &self.config)
            .field("users", &self.users.is_some())
            .finish_non_exhaustive()
    }
}

impl SchedulerService {
    pub fn new(
        config: SchedulerConfig,
        db_pool: &DbPool,
        app_context: Arc<AppContext>,
        schedule: Arc<AdmittedSchedule>,
    ) -> SchedulerResult<Self> {
        let repository = SchedulerRepository::new(db_pool);
        let state = app_context.kernel().state();
        let users = state
            .find::<systemprompt_traits::DynUserProvider>()
            .cloned();
        let kernel_pool = state.get::<DbPool>()?.clone();
        let instance_id = state.get::<Arc<Config>>()?.instance_id.clone();
        let system_admin_id = state.get::<SystemAdmin>()?.id().clone();
        let log_instance_id = state.get::<LoggingRuntime>()?.instance_id().cloned();
        let deployment = deployment_dependencies(state.get::<BootContext>()?);
        let background_tasks = state.get::<BackgroundTasks>()?.clone();
        let logging_repository = systemprompt_logging::LoggingRepository::new(db_pool);
        Ok(Self {
            config,
            repository,
            users,
            logging_repository,
            app_context,
            db_pool: kernel_pool,
            instance_id,
            system_admin_id,
            log_instance_id,
            deployment,
            background_tasks,
            schedule,
            clock: Arc::new(SystemClock),
        })
    }

    #[must_use]
    pub fn with_clock(mut self, clock: DynSchedulerClock) -> Self {
        self.clock = clock;
        self
    }

    pub async fn start(mut self) -> SchedulerResult<SchedulerStartup> {
        if !self.config.enabled {
            info!("Scheduler is disabled");
            return Ok(SchedulerStartup::disabled());
        }

        let registered_jobs = job_map(&self.schedule);
        self.validate_configured_jobs()?;
        self.adopt_unlisted_jobs(&registered_jobs)?;
        self.verify_lock_keys(&registered_jobs).await?;

        let resolved = self.resolve_owners(true).await?;
        self.check_requirements(&registered_jobs)?;

        debug!(
            admitted = registered_jobs.len(),
            configured = self.config.jobs.len(),
            "Admitted jobs resolved"
        );

        let running_jobs: RunningJobs = Arc::new(Mutex::new(HashSet::new()));

        let scheduler = JobScheduler::new().await?;
        let ctx = RegistrationCtx {
            scheduler: &scheduler,
            registered_jobs: &registered_jobs,
            running_jobs: &running_jobs,
            owners: resolved.owner_map(),
        };
        let outcome = self.register_jobs(&ctx).await;
        scheduler.start().await?;
        self.apply_missed_runs(&outcome.registered).await;

        let mut degraded = resolved.into_degraded();
        if !outcome.skipped.is_empty() {
            warn!(
                count = outcome.skipped.len(),
                "jobs were skipped because they could not be registered"
            );
            degraded.extend(outcome.skipped);
        }

        info!("Scheduler started");
        Ok(SchedulerStartup {
            handle: Some(SchedulerHandle { scheduler }),
            degraded,
            scheduled: outcome.registered.len(),
        })
    }

    fn check_requirements(&self, registered_jobs: &JobMap) -> SchedulerResult<()> {
        let deployment = &self.deployment;
        for job_config in self.config.jobs.iter().filter(|j| j.enabled) {
            let Some(job) = registered_jobs.get(job_config.name.as_str()) else {
                continue;
            };
            if !job.configured(&deployment) {
                continue;
            }
            let requirements = job.requires();
            if requirements.is_empty() {
                continue;
            }
            let state = job_dependencies(
                &job_config.name,
                job.db_budget(),
                &self.app_context,
                &self.db_pool,
                &self.schedule,
            );
            if let Some(unmet) = requirements.iter().find(|r| !r.is_met_by(&state)) {
                return Err(SchedulerError::UnmetRequirement {
                    job: job_config.name.clone(),
                    ty: unmet.type_name(),
                });
            }
        }
        Ok(())
    }

    async fn verify_lock_keys(&self, registered_jobs: &JobMap) -> SchedulerResult<()> {
        let mut names: Vec<JobName> = registered_jobs.keys().map(|n| JobName::new(*n)).collect();
        names.push(JobName::new(crate::repository::RECONCILE_LOCK));
        self.repository.verify_lock_keys(&names).await
    }

    fn validate_configured_jobs(&self) -> SchedulerResult<()> {
        let unknown = collect_unknown_job_names(&self.config, |name| {
            self.schedule.contains_job(name.as_str())
        });
        if unknown.is_empty() {
            Ok(())
        } else {
            Err(SchedulerError::UnknownJob {
                names: unknown
                    .iter()
                    .map(|name| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            })
        }
    }
}
