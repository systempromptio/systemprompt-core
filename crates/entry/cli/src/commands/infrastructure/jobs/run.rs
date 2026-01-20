use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use std::time::Instant;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext};

use super::types::{BatchJobRunOutput, JobRunOutput, JobRunResult};
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(help = "Job name(s) to run", num_args = 1..)]
    pub job_names: Vec<String>,

    #[arg(long, help = "Run all enabled jobs")]
    pub all: bool,

    #[arg(long, help = "Run jobs sequentially instead of in parallel")]
    pub sequential: bool,
}

pub async fn execute(args: RunArgs) -> Result<CommandResult<BatchJobRunOutput>> {
    let ctx = Arc::new(AppContext::new().await?);

    let job_names: Vec<String> = if args.all {
        inventory::iter::<&'static dyn Job>
            .into_iter()
            .filter(|j| j.enabled())
            .map(|j| j.name().to_string())
            .collect()
    } else if args.job_names.is_empty() {
        anyhow::bail!("Specify job name(s) or use --all to run all enabled jobs");
    } else {
        args.job_names
    };

    let mut results = Vec::new();

    for job_name in &job_names {
        let result = run_single_job(job_name, Arc::clone(&ctx)).await;
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

    Ok(CommandResult::table(output).with_title("Job Execution Results"))
}

async fn run_single_job(job_name: &str, ctx: Arc<AppContext>) -> JobRunOutput {
    let start = Instant::now();

    let job = inventory::iter::<&'static dyn Job>
        .into_iter()
        .find(|&j| j.name() == job_name)
        .copied();

    let Some(job) = job else {
        return JobRunOutput {
            job_name: job_name.to_string(),
            status: "failed".to_string(),
            duration_ms: start.elapsed().as_millis() as u64,
            result: JobRunResult {
                success: false,
                message: Some(format!("Unknown job: {}", job_name)),
                items_processed: None,
                items_failed: None,
            },
        };
    };

    let db_pool = Arc::clone(ctx.db_pool());
    let db_pool_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(db_pool);
    let app_context_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(Arc::clone(&ctx));
    let job_ctx = JobContext::new(db_pool_any, app_context_any);

    match job.execute(&job_ctx).await {
        Ok(result) => JobRunOutput {
            job_name: job_name.to_string(),
            status: if result.success { "success" } else { "failed" }.to_string(),
            duration_ms: start.elapsed().as_millis() as u64,
            result: JobRunResult {
                success: result.success,
                message: result.message,
                items_processed: result.items_processed,
                items_failed: result.items_failed,
            },
        },
        Err(e) => JobRunOutput {
            job_name: job_name.to_string(),
            status: "failed".to_string(),
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
