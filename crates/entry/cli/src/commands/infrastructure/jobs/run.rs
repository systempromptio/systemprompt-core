//! `jobs run` subcommand.
//!
//! Parses the CLI selection (names, `--all`, or `--tag`) and parameters,
//! delegates execution to
//! [`systemprompt_scheduler::JobExecutionService`], and renders the returned
//! batch report.
//!
//! A `cluster`-scoped job takes the same cross-replica advisory lock the
//! scheduler takes; `--lock-wait-secs` bounds how long the run waits for a
//! replica that holds it. A job skipped because another replica holds its
//! lock makes the command fail.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use clap::Args;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_identifiers::JobName;
use systemprompt_scheduler::{
    JobExecutionService, JobRunReport, JobSelection, parse_job_parameters,
};

use super::types::{BatchJobRunOutput, JobRunOutput, JobRunResult};
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(
        value_parser = crate::shared::parse_job_name,
        help = "Job name(s) to run",
        num_args = 1..
    )]
    pub job_names: Vec<JobName>,

    #[arg(long, help = "Run all enabled jobs")]
    pub all: bool,

    #[arg(long, help = "Run all jobs with the specified tag")]
    pub tag: Option<String>,

    #[arg(long, help = "Run jobs sequentially instead of in parallel")]
    pub sequential: bool,

    #[arg(
        long,
        short = 'y',
        help = "Skip confirmation when the resolved profile targets a remote/cloud environment"
    )]
    pub yes: bool,

    #[arg(
        long = "param",
        short = 'p',
        value_name = "KEY=VALUE",
        help = "Job parameters (can be specified multiple times)"
    )]
    pub params: Vec<String>,

    #[arg(
        long,
        default_value_t = 0,
        value_name = "SECONDS",
        help = "Wait up to this long for another replica holding a cluster job's lock (0 fails at once)"
    )]
    pub lock_wait_secs: u64,
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

    let scheduler_config = systemprompt_loader::ConfigLoader::load().map_or_else(
        |_| systemprompt_manifest::SchedulerConfig::with_system_admin(),
        |c| {
            c.scheduler
                .unwrap_or_else(systemprompt_manifest::SchedulerConfig::with_system_admin)
        },
    );
    let service = JobExecutionService::new(app, registry, scheduler_config)
        .with_lock_wait(Duration::from_secs(args.lock_wait_secs));
    let batch = service.run_jobs(&selection, &parameters).await?;

    if batch.contended > 0 {
        let held: Vec<&str> = batch
            .runs
            .iter()
            .filter(|run| run.lock_contended)
            .map(|run| run.job_name.as_str())
            .collect();
        bail!(
            "{} job(s) not run: another replica holds the distributed lock ({}); retry later or \
             pass --lock-wait-secs",
            held.len(),
            held.join(", ")
        );
    }

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
