//! On-demand job execution outside the cron loop.
//!
//! [`JobExecutionService`] resolves a [`JobSelection`] against both the
//! extension introspection manifest and the inventory catalog, runs each job
//! under a [`systemprompt_traits::JobContext`] actored to the system admin,
//! and records the outcome on the job's `scheduled_jobs` row. Callers render
//! the returned [`JobRunReport`] / [`JobBatchReport`]; a missing job is
//! reported as a failed run rather than an error so a batch keeps going.
//!
//! A manual run claims a job exactly as the cron loop does: with
//! `scheduler.distributed_lock` on, a `cluster`-scoped job runs only under
//! its cross-replica advisory lock, waiting up to the service's lock wait
//! (zero by default) for a replica that holds it. A run that cannot claim the
//! lock is reported as failed with [`JobRunReport::lock_contended`] set and
//! leaves the job's `scheduled_jobs` row to the replica that holds it.
//! `node`-scoped jobs, and every job without the distributed lock, run
//! unclaimed on this node.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod claim;
mod report;

pub use report::{JobBatchReport, JobRunReport, JobSelection, parse_job_parameters};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use systemprompt_extension::ExtensionRegistry;
use systemprompt_identifiers::{Actor, JobName};
use systemprompt_manifest::SchedulerConfig;
use systemprompt_runtime::AppContext;
use systemprompt_traits::Job;

use super::scheduling::make_job_context;
use crate::error::{SchedulerError, SchedulerResult};
use crate::models::{JobRunRecord, JobStatus};
use crate::repository::{JobRepository, SchedulerRepository};
use claim::Claimed;
use report::{elapsed_ms, failed_report};

enum RunnableJob {
    Manifest(Arc<dyn Job>),
    Inventory(&'static dyn Job),
}

impl RunnableJob {
    fn as_job(&self) -> &dyn Job {
        match self {
            Self::Manifest(job) => job.as_ref(),
            Self::Inventory(job) => *job,
        }
    }
}

pub struct JobExecutionService {
    ctx: Arc<AppContext>,
    registry: ExtensionRegistry,
    scheduler_config: SchedulerConfig,
    job_repo: JobRepository,
    scheduler_repo: SchedulerRepository,
    lock_wait: Duration,
}

impl std::fmt::Debug for JobExecutionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobExecutionService")
            .finish_non_exhaustive()
    }
}

impl JobExecutionService {
    pub fn new(
        ctx: Arc<AppContext>,
        registry: ExtensionRegistry,
        scheduler_config: SchedulerConfig,
    ) -> Self {
        let job_repo = JobRepository::new(ctx.db_pool());
        let scheduler_repo = SchedulerRepository::new(ctx.db_pool());
        Self {
            ctx,
            registry,
            scheduler_config,
            job_repo,
            scheduler_repo,
            lock_wait: Duration::ZERO,
        }
    }

    #[must_use]
    pub const fn with_lock_wait(mut self, wait: Duration) -> Self {
        self.lock_wait = wait;
        self
    }

    pub fn resolve_job_names(&self, selection: &JobSelection) -> SchedulerResult<Vec<JobName>> {
        match selection {
            JobSelection::All => {
                let mut names: Vec<JobName> = self
                    .registry
                    .all_jobs()
                    .into_iter()
                    .filter(|job| job.enabled())
                    .map(|job| JobName::new(job.name()))
                    .collect();
                for job in inventory::iter::<&'static dyn Job> {
                    if job.enabled() && !names.iter().any(|name| name == job.name()) {
                        names.push(JobName::new(job.name()));
                    }
                }
                names.sort_unstable();
                Ok(names)
            },
            JobSelection::Tag(tag) => {
                let jobs = self.registry.jobs_by_tag(tag);
                if jobs.is_empty() {
                    return Err(SchedulerError::NoJobsWithTag { tag: tag.clone() });
                }
                Ok(jobs
                    .into_iter()
                    .filter(|job| job.enabled())
                    .map(|job| JobName::new(job.name()))
                    .collect())
            },
            JobSelection::Names(names) => {
                if names.is_empty() {
                    return Err(SchedulerError::NoJobsSelected);
                }
                Ok(names.clone())
            },
        }
    }

    pub async fn run_jobs(
        &self,
        selection: &JobSelection,
        parameters: &HashMap<String, String>,
    ) -> SchedulerResult<JobBatchReport> {
        let job_names = self.resolve_job_names(selection)?;

        let mut runs = Vec::with_capacity(job_names.len());
        for job_name in &job_names {
            runs.push(self.run_job(job_name, parameters).await);
        }

        let succeeded = runs.iter().filter(|run| run.success).count();
        let failed = runs.len() - succeeded;
        let contended = runs.iter().filter(|run| run.lock_contended).count();
        Ok(JobBatchReport {
            runs,
            succeeded,
            failed,
            contended,
        })
    }

    pub async fn run_job(
        &self,
        job_name: &JobName,
        parameters: &HashMap<String, String>,
    ) -> JobRunReport {
        let start = Instant::now();

        let Some(runnable) = self.find_runnable(job_name) else {
            let report = failed_report(
                job_name,
                start,
                Some(format!("Job '{}' not found", job_name)),
            );
            self.record_run(&report).await;
            return report;
        };

        let guard = match self.claim(job_name, &runnable).await {
            Ok(Claimed::Held(guard)) => Some(guard),
            Ok(Claimed::Free) => None,
            Ok(Claimed::Contended) => return self.contended_report(job_name, start),
            Err(e) => {
                return failed_report(
                    job_name,
                    start,
                    Some(format!("Could not claim the job's distributed lock: {e}")),
                );
            },
        };

        let report = self
            .execute_runnable(job_name, &runnable, parameters, start)
            .await;
        self.record_run(&report).await;
        if let Some(guard) = guard {
            guard.release().await;
        }
        report
    }

    fn find_runnable(&self, job_name: &JobName) -> Option<RunnableJob> {
        if let Some(job) = self.registry.job_by_name(job_name.as_str()) {
            return Some(RunnableJob::Manifest(job));
        }
        inventory::iter::<&'static dyn Job>
            .into_iter()
            .find(|&job| job.name() == job_name)
            .copied()
            .map(RunnableJob::Inventory)
    }

    async fn execute_runnable(
        &self,
        job_name: &JobName,
        runnable: &RunnableJob,
        parameters: &HashMap<String, String>,
        start: Instant,
    ) -> JobRunReport {
        let admin_id = self.ctx.system_admin().id().clone();
        let actor = Actor::job(admin_id, job_name.as_str());
        let config_entry = self
            .scheduler_config
            .jobs
            .iter()
            .find(|job| &job.name == job_name);
        let enforce = config_entry.is_some_and(|job| job.enforce);
        let mut merged = config_entry
            .map(|job| job.parameters.clone())
            .unwrap_or_default();
        merged.extend(parameters.clone());
        let job_ctx =
            make_job_context(actor, Arc::clone(self.ctx.db_pool()), Arc::clone(&self.ctx))
                .with_enforce(enforce)
                .with_parameters(merged);

        match runnable.as_job().execute(&job_ctx).await {
            Ok(result) => JobRunReport {
                job_name: job_name.clone(),
                success: result.success,
                duration_ms: elapsed_ms(start),
                message: result.message,
                items_processed: result.items_processed,
                items_failed: result.items_failed,
                lock_contended: false,
            },
            Err(e) => failed_report(job_name, start, Some(e.to_string())),
        }
    }

    async fn record_run(&self, report: &JobRunReport) {
        let repo = &self.job_repo;

        let next_run = match repo.find_job(&report.job_name).await {
            Ok(Some(job)) => job.next_run,
            Ok(None) => return,
            Err(e) => {
                tracing::warn!(job = %report.job_name, error = %e, "could not look up scheduled job to record manual run");
                return;
            },
        };

        let (status, error) = if report.success {
            (JobStatus::Success, None)
        } else {
            (JobStatus::Failed, report.message.as_deref())
        };

        if let Err(e) = repo
            .update_job_execution(
                &report.job_name,
                JobRunRecord {
                    status,
                    error,
                    message: report
                        .success
                        .then_some(report.message.as_deref())
                        .flatten(),
                    next_run,
                    instance_id: &self.ctx.config().instance_id,
                },
            )
            .await
        {
            tracing::warn!(job = %report.job_name, error = %e, "failed to record manual job execution");
        }
    }
}
