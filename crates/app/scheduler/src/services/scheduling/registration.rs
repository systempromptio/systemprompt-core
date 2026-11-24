//! Job registration: turning configured jobs into cron entries.
//!
//! Registration is fault-isolated per job: a job that fails to upsert, parse
//! its schedule, or join the cron scheduler is collected as a [`SkippedJob`],
//! recorded as an `ERROR` in the `logs` table, and the remaining jobs are
//! registered as normal; one bad entry never stops the others or the boot.
//! A job whose `requires` is unmet is not such an entry: start refuses it
//! before registration (`SchedulerError::UnmetRequirement`).
//!
//! Each registered job is a [`Ticker`]: the cron loop fires it, it computes
//! the scheduled instant the tick belongs to from the scheduler's clock and
//! dispatches the job for that instant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::dispatch::Trigger;
use super::instants::{CronInstants, DynSchedulerClock};
use super::{RegistrationCtx, SchedulerService, dispatch};
use crate::error::{SchedulerError, SchedulerResult};
use crate::models::{JobConfig, SkippedJob};
use std::sync::Arc;
use systemprompt_identifiers::{Actor, SessionId, TraceId};
use systemprompt_logging::{LogActor, LogEntry, LogLevel, SystemSpan};
use systemprompt_traits::Job as JobTrait;
use tokio_cron_scheduler::Job;
use tracing::{Instrument, debug, info, warn};

enum Registered {
    Yes(Box<Ticker>),
    No,
}

pub(super) struct RegistrationOutcome {
    pub(super) registered: Vec<Ticker>,
    pub(super) skipped: Vec<SkippedJob>,
}

impl SchedulerService {
    pub(super) async fn register_jobs(&self, ctx: &RegistrationCtx<'_>) -> RegistrationOutcome {
        let mut registered = Vec::new();
        let mut skipped = Vec::new();
        for job_config in &self.config.jobs {
            match self.register_single_job(ctx, job_config).await {
                Ok(Registered::Yes(ticker)) => registered.push(*ticker),
                Ok(Registered::No) => {},
                Err(error) => {
                    warn!(
                        job = %job_config.name,
                        error = %error,
                        "job registration failed; continuing with the remaining jobs"
                    );
                    skipped.push(SkippedJob {
                        job_name: job_config.name.clone(),
                        owner: job_config
                            .owner
                            .as_ref()
                            .map_or_else(|| "system admin".to_owned(), |o| o.as_str().to_owned()),
                        reason: error.to_string(),
                    });
                },
            }
        }
        if !skipped.is_empty() {
            self.persist_registration_errors(&skipped).await;
        }
        RegistrationOutcome {
            registered,
            skipped,
        }
    }

    async fn persist_registration_errors(&self, skipped: &[SkippedJob]) {
        let repository = &self.logging_repository;
        let system_admin_id = self.system_admin_id.clone();
        for job in skipped {
            let actor = LogActor::new(
                system_admin_id.clone(),
                SessionId::system(),
                TraceId::generate(),
            );
            let entry = LogEntry::new(
                LogLevel::Error,
                "scheduler",
                format!(
                    "Job '{}' could not be registered and will not run: {}. \
                     Every other job was registered normally.",
                    job.job_name, job.reason
                ),
                actor,
            );
            let entry = match self.log_instance_id.as_ref() {
                Some(instance_id) => entry.with_instance_id(instance_id.clone()),
                None => entry,
            };
            if let Err(error) = repository.log(entry).await {
                warn!(error = %error, job_name = %job.job_name, "failed to persist scheduler registration error to logs");
            }
        }
    }

    async fn register_single_job(
        &self,
        ctx: &RegistrationCtx<'_>,
        job_config: &JobConfig,
    ) -> SchedulerResult<Registered> {
        if !job_config.enabled {
            debug!(job = %job_config.name, "Skipping disabled job");
            return Ok(Registered::No);
        }

        let Some(registered_job) = ctx.registered_jobs.get(job_config.name.as_str()) else {
            return Err(SchedulerError::job_not_found(job_config.name.clone()));
        };

        if !registered_job.configured(&self.deployment) {
            debug!(job = %job_config.name, "Job is not configured in this deployment; not scheduled");
            return Ok(Registered::No);
        }

        let Some(owner_id) = ctx.owners.get(&job_config.name).cloned() else {
            warn!(job = %job_config.name, "no resolved owner for job, skipping");
            return Ok(Registered::No);
        };
        let actor = Actor::job(owner_id, job_config.name.as_str());

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
            return Ok(Registered::No);
        }

        self.repository
            .upsert_job(&job_config.name, &schedule, job_config.enabled)
            .await?;

        let (job, ticker) = self.create_job_from_trait(ctx, job_config, &schedule, actor)?;
        ctx.scheduler.add(job).await?;
        Ok(Registered::Yes(Box::new(ticker)))
    }

    fn create_job_from_trait(
        &self,
        ctx: &RegistrationCtx<'_>,
        job_config: &JobConfig,
        schedule: &str,
        actor: Actor,
    ) -> SchedulerResult<(Job, Ticker)> {
        let registered_job: Arc<dyn JobTrait> = Arc::clone(
            ctx.registered_jobs
                .get(job_config.name.as_str())
                .ok_or_else(|| SchedulerError::job_not_found(job_config.name.clone()))?,
        );
        let claim_policy = dispatch::claim_policy(
            &self.config,
            Some(job_config),
            Some(registered_job.as_ref()),
            &self.instance_id,
        );
        let ticker = Ticker {
            dispatch: dispatch::JobDispatch {
                job: registered_job,
                job_name: job_config.name.clone(),
                actor,
                repository: self.repository.clone(),
                app_context: Arc::clone(&self.app_context),
                db_pool: Arc::clone(&self.db_pool),
                instance_id: self.instance_id.clone(),
                running_jobs: Arc::clone(ctx.running_jobs),
                claim_policy,
                enforce: job_config.enforce,
                parameters: job_config.parameters.clone(),
                schedule: Arc::clone(&self.schedule),
            },
            instants: CronInstants::parse(schedule)?,
            clock: Arc::clone(&self.clock),
        };

        let fired = ticker.clone();
        let job = Job::new_async(schedule, move |_uuid, _lock| {
            let ticker = fired.clone();
            Box::pin(async move { ticker.tick().await })
        })
        .map_err(SchedulerError::from)?;

        Ok((job, ticker))
    }
}

#[derive(Clone)]
pub(super) struct Ticker {
    pub(super) dispatch: dispatch::JobDispatch,
    pub(super) instants: CronInstants,
    pub(super) clock: DynSchedulerClock,
}

impl Ticker {
    async fn tick(self) {
        let job_name = self.dispatch.job_name.clone();
        let scheduled_at = match self.instants.at_or_before(self.clock.now()) {
            Ok(instant) => instant,
            Err(error) => {
                warn!(job = %job_name, %error, "tick has no scheduled instant; skipped");
                return;
            },
        };
        self.run(Trigger::Scheduled(scheduled_at)).await;
    }

    pub(super) async fn run(self, trigger: Trigger) {
        let span = SystemSpan::new(&format!("scheduler:{}", self.dispatch.job_name));
        dispatch::execute_job(self.dispatch, trigger)
            .instrument(span.span().clone())
            .await;
    }
}
