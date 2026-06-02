use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::JobRepository;

use super::types::{JobHistoryEntry, JobHistoryOutput};
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct HistoryArgs {
    #[arg(long, help = "Filter by job name")]
    pub job: Option<String>,

    #[arg(
        long,
        short = 'n',
        default_value = "20",
        help = "Number of entries to show"
    )]
    pub limit: i64,

    #[arg(long, help = "Filter by status (success, failed, running)")]
    pub status: Option<String>,
}

pub(super) async fn execute(args: HistoryArgs) -> Result<CommandOutput> {
    let ctx = Arc::new(AppContext::new().await?);
    let repo = JobRepository::new(ctx.db_pool())?;

    let entries: Vec<JobHistoryEntry> = if let Some(ref job_name) = args.job {
        match repo.find_job(job_name).await? {
            Some(j) => j.last_run.map_or_else(Vec::new, |last_run| {
                vec![JobHistoryEntry {
                    job_name: j.job_name,
                    status: j.last_status.unwrap_or_else(|| "unknown".to_owned()),
                    run_at: last_run,
                    error: j.last_error,
                }]
            }),
            None => vec![],
        }
    } else {
        repo.list_recent_runs(args.limit)
            .await?
            .into_iter()
            .filter_map(|j| {
                j.last_run.map(|last_run| JobHistoryEntry {
                    job_name: j.job_name,
                    status: j.last_status.unwrap_or_else(|| "unknown".to_owned()),
                    run_at: last_run,
                    error: j.last_error,
                })
            })
            .filter(|e| {
                args.status
                    .as_ref()
                    .is_none_or(|s| e.status.eq_ignore_ascii_case(s))
            })
            .collect()
    };

    let total = entries.len();
    let output = JobHistoryOutput { entries, total };

    Ok(CommandOutput::table_of(
        vec!["job_name", "status", "run_at", "error"],
        &output.entries,
    )
    .with_title("Job Execution History"))
}
