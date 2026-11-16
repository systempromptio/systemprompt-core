//! `infra jobs prune` command retiring `scheduled_jobs` rows.
//!
//! A node's boot never deletes a `scheduled_jobs` row, so a job no
//! composition admits any longer keeps its row until this command removes it.
//! The command lists the rows no job of the admitted composition names and,
//! unless `--dry-run` is given, deletes exactly those rows after the operator
//! confirms. The rows of a member this profile disables are included, because
//! a disabled member is not admitted. It judges against this composition
//! alone, so it is run with a binary and profile that admit every capability
//! the cluster runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::{Context, Result};
use clap::Args;
use systemprompt_identifiers::JobName;
use systemprompt_scheduler::{JobPruneService, JobRepository, ScheduledJob};

use super::admitted_schedule;
use super::types::PrunedJob;
use systemprompt_database::pool_from_handle;
use systemprompt_extension::cli::require_confirmation;
use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct PruneArgs {
    #[arg(
        long,
        help = "List the rows that would be deleted without deleting them"
    )]
    pub dry_run: bool,

    #[arg(short = 'y', long, help = "Skip the confirmation prompt")]
    pub yes: bool,
}

pub(super) async fn execute(args: PruneArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    let schedule = admitted_schedule(host).await?;
    let service = JobPruneService::new(
        JobRepository::new(&pool_from_handle(host.db().await?)?),
        &schedule,
    );
    let rows = service
        .unregistered()
        .await
        .context("Failed to list unregistered scheduled_jobs rows")?;
    let names: Vec<JobName> = rows.iter().map(|row| row.job_name.clone()).collect();
    let jobs: Vec<PrunedJob> = rows.into_iter().map(pruned_job).collect();

    if args.dry_run || jobs.is_empty() {
        return Ok(table(&jobs, 0, args.dry_run));
    }

    require_confirmation(
        host.prompter(),
        &format!(
            "Delete {} scheduled_jobs row(s) no admitted job names? A disabled \
             extension's rows are included. This cannot be undone.",
            names.len()
        ),
        args.yes,
        host.output(),
    )?;

    let deleted = service
        .delete(&names)
        .await
        .context("Failed to delete unregistered scheduled_jobs rows")?;
    Ok(table(&jobs, deleted, false))
}

fn pruned_job(row: ScheduledJob) -> PrunedJob {
    PrunedJob {
        job_name: row.job_name,
        schedule: row.schedule,
        enabled: row.enabled,
        last_run: row.last_run,
        run_count: row.run_count,
    }
}

fn table(jobs: &[PrunedJob], deleted: u64, dry_run: bool) -> CommandOutput {
    let title = if dry_run {
        format!(
            "Unregistered Jobs (dry run, {} would be deleted)",
            jobs.len()
        )
    } else {
        format!("Pruned Jobs ({deleted} deleted)")
    };
    CommandOutput::table_of(
        vec!["job_name", "schedule", "enabled", "last_run", "run_count"],
        jobs,
    )
    .with_title(title)
}
