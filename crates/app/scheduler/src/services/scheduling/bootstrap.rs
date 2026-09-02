//! Run-once bootstrap job execution at scheduler start.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use systemprompt_identifiers::{Actor, InstanceId, UserId};
use systemprompt_traits::{Job as JobTrait, OptionalStartupEventExt, StartupEventSender};
use tokio::sync::Mutex;

use super::{RunningJobs, SchedulerService, dispatch};
use crate::error::SchedulerResult;
use crate::models::JobStatus;

struct BootstrapCtx<'a> {
    owners: &'a HashMap<String, UserId>,
    skipped: &'a HashSet<&'a str>,
    system_admin_id: &'a UserId,
    running_jobs: &'a RunningJobs,
    events: Option<&'a StartupEventSender>,
}

impl SchedulerService {
    pub async fn run_bootstrap_jobs(
        &self,
        events: Option<&StartupEventSender>,
    ) -> SchedulerResult<usize> {
        let resolved = self.resolve_owners(false).await?;
        let registered_jobs = Self::discover_jobs();
        self.validate_configured_jobs(&registered_jobs)?;
        let system_admin_id = self.app_context.system_admin().id().clone();
        let skipped: HashSet<&str> = resolved.skipped_names().collect();
        let running_jobs: RunningJobs = Arc::new(Mutex::new(HashSet::new()));
        let ctx = BootstrapCtx {
            owners: resolved.owner_map(),
            skipped: &skipped,
            system_admin_id: &system_admin_id,
            running_jobs: &running_jobs,
            events,
        };

        for job_name in &self.config.bootstrap_jobs {
            self.dispatch_bootstrap_job(job_name, &ctx).await;
        }

        Ok(registered_jobs.len())
    }

    async fn dispatch_bootstrap_job(&self, job_name: &str, ctx: &BootstrapCtx<'_>) {
        if ctx.skipped.contains(job_name) {
            tracing::warn!(job_name = %job_name, "bootstrap job owner unresolved, skipping");
            return;
        }
        let owner_id = ctx
            .owners
            .get(job_name)
            .cloned()
            .unwrap_or_else(|| ctx.system_admin_id.clone());
        let actor = Actor::job(owner_id, job_name.to_owned());

        ctx.events.bootstrap_job_started(job_name.to_owned());

        let config_entry = self.config.jobs.iter().find(|job| job.name == job_name);
        let registered = inventory::iter::<&'static dyn JobTrait>
            .into_iter()
            .find(|job| job.name() == job_name)
            .copied();
        let claim_policy = dispatch::claim_policy(
            &self.config,
            config_entry,
            registered,
            &InstanceId::new(&self.app_context.config().instance_id),
        );
        dispatch::execute_job(dispatch::JobDispatch {
            job_name: job_name.to_owned(),
            actor,
            db_pool: Arc::clone(&self.db_pool),
            repository: self.repository.clone(),
            app_context: Arc::clone(&self.app_context),
            running_jobs: Arc::clone(ctx.running_jobs),
            claim_policy,
            enforce: config_entry.is_some_and(|job| job.enforce),
            parameters: config_entry
                .map(|job| job.parameters.clone())
                .unwrap_or_default(),
        })
        .await;

        let (success, message) = match self.repository.find_job(job_name).await {
            Ok(Some(row)) => {
                let succeeded = row.last_status.as_deref() == Some(JobStatus::Success.as_str());
                (succeeded, row.last_error)
            },
            Ok(None) => (false, Some("job row missing after dispatch".to_owned())),
            Err(e) => (false, Some(e.to_string())),
        };

        ctx.events
            .bootstrap_job_completed(job_name.to_owned(), success, message);
    }
}
