//! `jobs run` subcommand.
//!
//! Runs one or more jobs on demand, selected by name, `--all`, or `--tag`,
//! resolving both registry-declared and inventory-registered jobs. Each job
//! executes under a [`JobContext`] actored to the admin owner, collecting
//! per-job success/failure into a batch result.

use anyhow::Result;
use clap::Args;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{JobRepository, JobStatus};
use systemprompt_traits::{Job, JobContext};

use super::types::{BatchJobRunOutput, JobRunOutput, JobRunResult};
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(help = "Job name(s) to run", num_args = 1..)]
    pub job_names: Vec<String>,

    #[arg(long, help = "Run all enabled jobs")]
    pub all: bool,

    #[arg(long, help = "Run all jobs with the specified tag")]
    pub tag: Option<String>,

    #[arg(long, help = "Run jobs sequentially instead of in parallel")]
    pub sequential: bool,

    #[arg(
        long = "param",
        short = 'p',
        value_name = "KEY=VALUE",
        help = "Job parameters (can be specified multiple times)"
    )]
    pub params: Vec<String>,
}

pub(super) async fn execute(args: RunArgs) -> Result<CommandOutput> {
    let ctx = Arc::new(AppContext::new().await?);
    let registry = ExtensionRegistry::discover()?;

    let parameters = parse_params(&args.params)?;

    let job_names: Vec<String> = if args.all {
        let mut names: Vec<String> = registry
            .all_jobs()
            .into_iter()
            .filter(|j| j.enabled())
            .map(|j| j.name().to_owned())
            .collect();

        for job in inventory::iter::<&'static dyn Job> {
            if job.enabled() && !names.contains(&job.name().to_owned()) {
                names.push(job.name().to_owned());
            }
        }
        names
    } else if let Some(tag) = &args.tag {
        let jobs = registry.jobs_by_tag(tag);
        if jobs.is_empty() {
            anyhow::bail!("No jobs found with tag '{}'", tag);
        }
        jobs.into_iter()
            .filter(|j| j.enabled())
            .map(|j| j.name().to_owned())
            .collect()
    } else if args.job_names.is_empty() {
        anyhow::bail!("Specify job name(s), use --all, or use --tag <tag> to run jobs");
    } else {
        args.job_names
    };

    let mut results = Vec::new();

    for job_name in &job_names {
        let result = run_single_job(job_name, Arc::clone(&ctx), &registry, &parameters).await;
        record_job_execution(&ctx, &result).await;
        results.push(result);
    }

    let succeeded = results.iter().filter(|r| r.result.success).count();
    let failed = results.len() - succeeded;

    let output = BatchJobRunOutput {
        total: results.len(),
        succeeded,
        failed,
        jobs_run: results,
    };

    Ok(CommandOutput::table_of(
        vec!["job_name", "status", "duration_ms", "result"],
        &output.jobs_run,
    )
    .with_title("Job Execution Results"))
}

fn parse_params(params: &[String]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for param in params {
        let parts: Vec<&str> = param.splitn(2, '=').collect();
        if parts.len() != 2 {
            anyhow::bail!(
                "Invalid parameter format '{}'. Use KEY=VALUE format.",
                param
            );
        }
        map.insert(parts[0].to_owned(), parts[1].to_owned());
    }
    Ok(map)
}

/// Record a manual job run into `scheduled_jobs` so it surfaces in
/// `infra jobs history`, mirroring how the scheduler records its own ticks.
///
/// The update only touches a job that already has a `scheduled_jobs` row
/// (manual-only jobs have none, and the UPDATE is a harmless no-op for them).
/// The job's existing `next_run` is preserved so recording a manual run never
/// disturbs the schedule. Failures here are non-fatal: recording is bookkeeping
/// and must not mask the job's own outcome, so errors are logged and swallowed.
async fn record_job_execution(ctx: &AppContext, output: &JobRunOutput) {
    let repo = match JobRepository::new(ctx.db_pool()) {
        Ok(repo) => repo,
        Err(e) => {
            tracing::warn!(job = %output.job_name, error = %e, "could not open scheduler repo to record manual run");
            return;
        },
    };

    // Preserve the existing schedule; a manual run must not clear next_run.
    let next_run = match repo.find_job(&output.job_name).await {
        Ok(Some(job)) => job.next_run,
        Ok(None) => return, // not a scheduled job — nothing to record
        Err(e) => {
            tracing::warn!(job = %output.job_name, error = %e, "could not look up scheduled job to record manual run");
            return;
        },
    };

    let (status, error) = if output.result.success {
        (JobStatus::Success, None)
    } else {
        (JobStatus::Failed, output.result.message.as_deref())
    };

    if let Err(e) = repo
        .update_job_execution(&output.job_name, status, error, next_run)
        .await
    {
        tracing::warn!(job = %output.job_name, error = %e, "failed to record manual job execution");
        return;
    }
    if let Err(e) = repo.increment_run_count(&output.job_name).await {
        tracing::warn!(job = %output.job_name, error = %e, "failed to increment job run count");
    }
}

async fn run_single_job(
    job_name: &str,
    ctx: Arc<AppContext>,
    registry: &ExtensionRegistry,
    parameters: &HashMap<String, String>,
) -> JobRunOutput {
    let start = Instant::now();

    let ext_job = registry.job_by_name(job_name);

    let inv_job = inventory::iter::<&'static dyn Job>
        .into_iter()
        .find(|&j| j.name() == job_name)
        .copied();

    if ext_job.is_none() && inv_job.is_none() {
        return JobRunOutput {
            job_name: job_name.to_owned(),
            status: "failed".to_owned(),
            duration_ms: start.elapsed().as_millis() as u64,
            result: JobRunResult {
                success: false,
                message: Some(format!("Job '{}' not found", job_name)),
                items_processed: None,
                items_failed: None,
            },
        };
    }

    let db_pool = Arc::clone(ctx.db_pool());
    let users = match systemprompt_users::UserRepository::new(&db_pool) {
        Ok(users) => users,
        Err(e) => {
            return JobRunOutput {
                job_name: job_name.to_owned(),
                status: "failed".to_owned(),
                duration_ms: start.elapsed().as_millis() as u64,
                result: JobRunResult {
                    success: false,
                    message: Some(format!("failed to open users repository: {e}")),
                    items_processed: None,
                    items_failed: None,
                },
            };
        },
    };
    let Ok(Some(admin_user)) = users.find_admin_owner().await else {
        return JobRunOutput {
            job_name: job_name.to_owned(),
            status: "failed".to_owned(),
            duration_ms: start.elapsed().as_millis() as u64,
            result: JobRunResult {
                success: false,
                message: Some(
                    "no user with role 'admin' exists; create one with `systemprompt admin users \
                     create --role admin <name>` before running ad-hoc jobs"
                        .to_owned(),
                ),
                items_processed: None,
                items_failed: None,
            },
        };
    };
    let actor = systemprompt_identifiers::Actor::job(admin_user.id, job_name.to_owned());
    let db_pool_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(db_pool);
    let app_paths_any: Arc<dyn std::any::Any + Send + Sync> =
        Arc::new(Arc::clone(ctx.app_paths_arc()));
    let app_context_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(Arc::clone(&ctx));
    let job_ctx = JobContext::new(actor, db_pool_any, app_context_any, app_paths_any)
        .with_parameters(parameters.clone());

    let execute_result = if let Some(job) = ext_job {
        job.execute(&job_ctx).await
    } else if let Some(job) = inv_job {
        job.execute(&job_ctx).await
    } else {
        unreachable!()
    };

    match execute_result {
        Ok(result) => JobRunOutput {
            job_name: job_name.to_owned(),
            status: if result.success { "success" } else { "failed" }.to_owned(),
            duration_ms: start.elapsed().as_millis() as u64,
            result: JobRunResult {
                success: result.success,
                message: result.message,
                items_processed: result.items_processed,
                items_failed: result.items_failed,
            },
        },
        Err(e) => JobRunOutput {
            job_name: job_name.to_owned(),
            status: "failed".to_owned(),
            duration_ms: start.elapsed().as_millis() as u64,
            result: JobRunResult {
                success: false,
                message: Some(e.to_string()),
                items_processed: None,
                items_failed: None,
            },
        },
    }
}
