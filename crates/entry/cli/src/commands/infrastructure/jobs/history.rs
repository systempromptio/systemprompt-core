use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::JobRepository;

use super::types::{JobHistoryEntry, JobHistoryOutput};
use crate::shared::{CommandResult, RenderingHints};

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

pub async fn execute(args: HistoryArgs) -> Result<CommandResult<JobHistoryOutput>> {
    let ctx = Arc::new(AppContext::new().await?);
    let repo = JobRepository::new(ctx.db_pool())?;

    let entries: Vec<JobHistoryEntry> = if let Some(ref job_name) = args.job {
        match repo.find_job(job_name).await? {
            Some(j) => j.last_run.map_or_else(Vec::new, |last_run| {
                vec![JobHistoryEntry {
                    job_name: j.job_name,
                    status: j.last_status.unwrap_or_else(|| "unknown".to_string()),
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
                    status: j.last_status.unwrap_or_else(|| "unknown".to_string()),
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

    Ok(CommandResult::table(output)
        .with_title("Job Execution History")
        .with_hints(RenderingHints {
            columns: Some(vec![
                "job_name".to_string(),
                "status".to_string(),
                "run_at".to_string(),
                "error".to_string(),
            ]),
            ..Default::default()
        }))
}
