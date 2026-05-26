use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use systemprompt_identifiers::{Actor, UserId};
use systemprompt_traits::{OptionalStartupEventExt, StartupEventSender};
use tokio::sync::Mutex;
use tracing::warn;

use super::{RunningJobs, SchedulerService, dispatch};
use crate::error::SchedulerResult;
use crate::models::JobStatus;

impl SchedulerService {
    /// Runs the configured `bootstrap_jobs` once, serially, at startup —
    /// independent of the cron schedule used by [`Self::start`]. Jobs absent
    /// from the inventory are skipped with a warning. Emits start/complete
    /// events through `events` when provided, and returns the number of jobs
    /// discovered in the inventory.
    pub async fn run_bootstrap_jobs(
        &self,
        events: Option<&StartupEventSender>,
    ) -> SchedulerResult<usize> {
        let resolved_owners = Self::resolve_owners(&self.db_pool, &self.config.jobs).await?;
        let registered_jobs = Self::discover_jobs();
        let running_jobs: RunningJobs = Arc::new(Mutex::new(HashSet::new()));

        for job_name in &self.config.bootstrap_jobs {
            if !registered_jobs.contains_key(job_name.as_str()) {
                warn!(job = %job_name, "Bootstrap job not found in inventory, skipping");
                continue;
            }
            self.dispatch_bootstrap_job(job_name, &resolved_owners, &running_jobs, events)
                .await;
        }

        Ok(registered_jobs.len())
    }

    // Why: `running_jobs` is cloned into `JobDispatch` and read by
    // `execute_job` across an `.await`; clippy's intra-function analysis
    // cannot see that read and flags the param as write-only.
    #[expect(
        clippy::collection_is_never_read,
        reason = "`running_jobs` is cloned into JobDispatch and read by execute_job across an \
                  .await; intra-function clippy analysis cannot see that read"
    )]
    async fn dispatch_bootstrap_job(
        &self,
        job_name: &str,
        owners: &HashMap<String, UserId>,
        running_jobs: &RunningJobs,
        events: Option<&StartupEventSender>,
    ) {
        let Some(owner_id) = owners.get(job_name).cloned() else {
            warn!(job = %job_name, "Bootstrap job has no resolved owner; skipping");
            return;
        };
        let actor = Actor::job(owner_id, job_name.to_owned());

        events.bootstrap_job_started(job_name.to_owned());

        dispatch::execute_job(dispatch::JobDispatch {
            job_name: job_name.to_owned(),
            actor,
            db_pool: Arc::clone(&self.db_pool),
            repository: self.repository.clone(),
            app_context: Arc::clone(&self.app_context),
            running_jobs: Arc::clone(running_jobs),
            distributed_lock: self.config.distributed_lock,
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

        events.bootstrap_job_completed(job_name.to_owned(), success, message);
    }
}
