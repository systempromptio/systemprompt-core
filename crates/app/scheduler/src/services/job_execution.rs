//! On-demand job execution outside the cron loop.
//!
//! [`JobExecutionService`] resolves a [`JobSelection`] against both the
//! extension introspection manifest and the inventory catalog, runs each job
//! under a [`systemprompt_traits::JobContext`] actored to the system admin,
//! and records the outcome on the job's `scheduled_jobs` row. Callers render
//! the returned [`JobRunReport`] / [`JobBatchReport`]; a missing job is
//! reported as a failed run rather than an error so a batch keeps going.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use systemprompt_extension::ExtensionRegistry;
use systemprompt_identifiers::Actor;
use systemprompt_runtime::AppContext;
use systemprompt_traits::Job;

use super::scheduling::make_job_context;
use crate::error::{SchedulerError, SchedulerResult};
use crate::models::JobStatus;
use crate::repository::JobRepository;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobSelection {
    All,
    Tag(String),
    Names(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct JobRunReport {
    pub job_name: String,
    pub success: bool,
    pub duration_ms: u64,
    pub message: Option<String>,
    pub items_processed: Option<u64>,
    pub items_failed: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct JobBatchReport {
    pub runs: Vec<JobRunReport>,
    pub succeeded: usize,
    pub failed: usize,
}

pub fn parse_job_parameters(params: &[String]) -> SchedulerResult<HashMap<String, String>> {
    let mut map = HashMap::new();
    for param in params {
        let Some((key, value)) = param.split_once('=') else {
            return Err(SchedulerError::InvalidJobParameter {
                parameter: param.clone(),
            });
        };
        map.insert(key.to_owned(), value.to_owned());
    }
    Ok(map)
}

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
}

impl std::fmt::Debug for JobExecutionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobExecutionService")
            .finish_non_exhaustive()
    }
}

impl JobExecutionService {
    #[must_use]
    pub const fn new(ctx: Arc<AppContext>, registry: ExtensionRegistry) -> Self {
        Self { ctx, registry }
    }

    pub fn resolve_job_names(&self, selection: &JobSelection) -> SchedulerResult<Vec<String>> {
        match selection {
            JobSelection::All => {
                let mut names: Vec<String> = self
                    .registry
                    .all_jobs()
                    .into_iter()
                    .filter(|job| job.enabled())
                    .map(|job| job.name().to_owned())
                    .collect();
                for job in inventory::iter::<&'static dyn Job> {
                    if job.enabled() && !names.iter().any(|name| name == job.name()) {
                        names.push(job.name().to_owned());
                    }
                }
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
                    .map(|job| job.name().to_owned())
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
        Ok(JobBatchReport {
            runs,
            succeeded,
            failed,
        })
    }

    pub async fn run_job(
        &self,
        job_name: &str,
        parameters: &HashMap<String, String>,
    ) -> JobRunReport {
        let start = Instant::now();

        let report = match self.find_runnable(job_name) {
            Some(runnable) => {
                self.execute_runnable(job_name, &runnable, parameters, start)
                    .await
            },
            None => failed_report(
                job_name,
                start,
                Some(format!("Job '{}' not found", job_name)),
            ),
        };

        self.record_run(&report).await;
        report
    }

    fn find_runnable(&self, job_name: &str) -> Option<RunnableJob> {
        if let Some(job) = self.registry.job_by_name(job_name) {
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
        job_name: &str,
        runnable: &RunnableJob,
        parameters: &HashMap<String, String>,
        start: Instant,
    ) -> JobRunReport {
        let admin_id = self.ctx.system_admin().id().clone();
        let actor = Actor::job(admin_id, job_name.to_owned());
        let job_ctx =
            make_job_context(actor, Arc::clone(self.ctx.db_pool()), Arc::clone(&self.ctx))
                .with_parameters(parameters.clone());

        match runnable.as_job().execute(&job_ctx).await {
            Ok(result) => JobRunReport {
                job_name: job_name.to_owned(),
                success: result.success,
                duration_ms: elapsed_ms(start),
                message: result.message,
                items_processed: result.items_processed,
                items_failed: result.items_failed,
            },
            Err(e) => failed_report(job_name, start, Some(e.to_string())),
        }
    }

    async fn record_run(&self, report: &JobRunReport) {
        let repo = match JobRepository::new(self.ctx.db_pool()) {
            Ok(repo) => repo,
            Err(e) => {
                tracing::warn!(job = %report.job_name, error = %e, "could not open scheduler repo to record manual run");
                return;
            },
        };

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
            .update_job_execution(&report.job_name, status, error, next_run)
            .await
        {
            tracing::warn!(job = %report.job_name, error = %e, "failed to record manual job execution");
            return;
        }
        if let Err(e) = repo.increment_run_count(&report.job_name).await {
            tracing::warn!(job = %report.job_name, error = %e, "failed to increment job run count");
        }
    }
}

fn failed_report(job_name: &str, start: Instant, message: Option<String>) -> JobRunReport {
    JobRunReport {
        job_name: job_name.to_owned(),
        success: false,
        duration_ms: elapsed_ms(start),
        message,
        items_processed: None,
        items_failed: None,
    }
}

fn elapsed_ms(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}
