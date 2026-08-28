//! Job registration: turning configured jobs into cron entries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{RegistrationCtx, RunningJobs, SchedulerService, dispatch};
use crate::error::{SchedulerError, SchedulerResult};
use crate::models::JobConfig;
use std::sync::Arc;
use systemprompt_identifiers::Actor;
use systemprompt_logging::SystemSpan;
use tokio_cron_scheduler::Job;
use tracing::{Instrument, debug, info, warn};

impl SchedulerService {
    pub(super) async fn register_jobs(&self, ctx: &RegistrationCtx<'_>) -> SchedulerResult<()> {
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
        let parameters = job_config.parameters.clone();
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
            let parameters = parameters.clone();

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
