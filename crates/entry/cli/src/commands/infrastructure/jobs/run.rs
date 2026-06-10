//! `jobs run` subcommand.
//!
//! Parses the CLI selection (names, `--all`, or `--tag`) and parameters,
//! delegates execution to
//! [`systemprompt_scheduler::JobExecutionService`], and renders the returned
//! batch report.

use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_scheduler::{
    JobExecutionService, JobRunReport, JobSelection, parse_job_parameters,
};

use super::types::{BatchJobRunOutput, JobRunOutput, JobRunResult};
use crate::context::CommandContext;
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

pub(super) async fn execute(args: RunArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let app = Arc::clone(ctx.app_context().await?);
    let registry = ExtensionRegistry::discover()?;

    let parameters = parse_job_parameters(&args.params)?;
    let selection = if args.all {
        JobSelection::All
    } else if let Some(tag) = args.tag {
        JobSelection::Tag(tag)
    } else {
        JobSelection::Names(args.job_names)
    };

    let service = JobExecutionService::new(app, registry);
    let batch = service.run_jobs(&selection, &parameters).await?;

    let jobs_run: Vec<JobRunOutput> = batch.runs.into_iter().map(into_output).collect();
    let output = BatchJobRunOutput {
        total: jobs_run.len(),
        succeeded: batch.succeeded,
        failed: batch.failed,
        jobs_run,
    };

    Ok(CommandOutput::table_of(
        vec!["job_name", "status", "duration_ms", "result"],
        &output.jobs_run,
    )
    .with_title("Job Execution Results"))
}

fn into_output(report: JobRunReport) -> JobRunOutput {
    JobRunOutput {
        job_name: report.job_name,
        status: if report.success { "success" } else { "failed" }.to_owned(),
        duration_ms: report.duration_ms,
        result: JobRunResult {
            success: report.success,
            message: report.message,
            items_processed: report.items_processed,
            items_failed: report.items_failed,
        },
    }
}
