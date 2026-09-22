//! Job registration: turning configured jobs into cron entries.
//!
//! Registration is fault-isolated per job: a job that fails to upsert, parse
//! its schedule, or join the cron scheduler is collected as a [`SkippedJob`],
//! recorded as an `ERROR` in the `logs` table, and the remaining jobs are
//! registered as normal. Before this, one bad entry aborted the loop and every
//! later job was silently dropped — and, because the error reached the server
//! lifecycle, took the whole boot with it.
//!
//! `Registered::NotInInventory` is separated from the other skips because it
//! is the only one that means the operator is looking at a stale deployment:
//! the configuration names a job this binary was not built with. It is
//! reported, not merely logged, because the registration warning never
//! reaches the `logs` table — the database log layer drops any event whose
//! span carries no user, session and trace, and boot-time registration runs
//! outside one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{RegistrationCtx, SchedulerService, dispatch};
use crate::error::{SchedulerError, SchedulerResult};
use crate::models::{JobConfig, SkippedJob};
use std::sync::Arc;
use systemprompt_identifiers::{Actor, InstanceId, SessionId, TraceId};
use systemprompt_logging::{LogActor, LogEntry, LogLevel, SystemSpan};
use systemprompt_traits::Job as JobTrait;
use tokio_cron_scheduler::Job;
use tracing::{Instrument, debug, info, warn};

enum Registered {
    Yes,
    No,
    NotInInventory,
}

pub(super) struct RegistrationOutcome {
    pub(super) registered: usize,
    pub(super) skipped: Vec<SkippedJob>,
}

impl SchedulerService {
    pub(super) async fn register_jobs(&self, ctx: &RegistrationCtx<'_>) -> RegistrationOutcome {
        let mut registered = 0;
        let mut skipped = Vec::new();
        for job_config in &self.config.jobs {
            match self.register_single_job(ctx, job_config).await {
                Ok(Registered::Yes) => registered += 1,
                Ok(Registered::NotInInventory) => skipped.push(SkippedJob {
                    job_name: job_config.name.clone(),
                    owner: job_config
                        .owner
                        .as_ref()
                        .map_or_else(|| "system admin".to_owned(), |o| o.as_str().to_owned()),
                    reason: "this build has no job by that name; the configuration is ahead of                              the deployed binary"
                        .to_owned(),
                }),
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
        let system_admin_id = self.app_context.system_admin().id().clone();
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
            warn!(job = %job_config.name, "Job not found in inventory, skipping");
            return Ok(Registered::NotInInventory);
        };

        let Some(owner_id) = ctx.owners.get(&job_config.name).cloned() else {
            warn!(job = %job_config.name, "no resolved owner for job, skipping");
            return Ok(Registered::No);
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
            return Ok(Registered::No);
        }

        self.repository
            .upsert_job(&job_config.name, &schedule, job_config.enabled)
            .await?;

        let job = self.create_job_from_trait(ctx, job_config, &schedule, actor)?;
        ctx.scheduler.add(job).await?;
        Ok(Registered::Yes)
    }

    fn create_job_from_trait(
        &self,
        ctx: &RegistrationCtx<'_>,
        job_config: &JobConfig,
        schedule: &str,
        actor: Actor,
    ) -> SchedulerResult<Job> {
        let registered_job: &dyn JobTrait = *ctx
            .registered_jobs
            .get(job_config.name.as_str())
            .ok_or_else(|| SchedulerError::job_not_found(&job_config.name))?;
        let running_jobs = ctx.running_jobs;
        let enforce = job_config.enforce;
        let parameters = job_config.parameters.clone();
        let job_name_owned = job_config.name.clone();
        let schedule_owned = schedule.to_owned();
        let db_pool = Arc::clone(&self.db_pool);
        let repository = self.repository.clone();
        let app_context = Arc::clone(&self.app_context);
        let running_jobs = Arc::clone(running_jobs);
        let claim_policy = dispatch::claim_policy(
            &self.config,
            Some(job_config),
            Some(registered_job),
            &InstanceId::new(&self.app_context.config().instance_id),
        );

        let job = Job::new_async(schedule_owned.as_str(), move |_uuid, _lock| {
            let job_name = job_name_owned.clone();
            let actor = actor.clone();
            let db_pool = Arc::clone(&db_pool);
            let repository = repository.clone();
            let app_context = Arc::clone(&app_context);
            let running_jobs = Arc::clone(&running_jobs);
            let parameters = parameters.clone();
            let claim_policy = claim_policy.clone();

            Box::pin(async move {
                let span = SystemSpan::new(&format!("scheduler:{job_name}"));
                dispatch::execute_job(dispatch::JobDispatch {
                    job_name,
                    actor,
                    db_pool,
                    repository,
                    app_context,
                    running_jobs,
                    claim_policy,
                    enforce,
                    parameters,
                })
                .instrument(span.span().clone())
                .await;
            })
        })
        .map_err(SchedulerError::from)?;

        Ok(job)
    }
}
