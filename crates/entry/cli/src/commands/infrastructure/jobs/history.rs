use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_core_scheduler::ScheduledJob;
use systemprompt_runtime::AppContext;

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
    let pool = ctx.db_pool().pool_arc()?;

    let entries: Vec<JobHistoryEntry> = if let Some(ref job_name) = args.job {
        let job = sqlx::query_as!(
            ScheduledJob,
            r#"
            SELECT id, job_name, schedule, enabled, last_run, next_run,
                   last_status, last_error, run_count, created_at, updated_at
            FROM scheduled_jobs
            WHERE job_name = $1
            "#,
            job_name
        )
        .fetch_optional(&*pool)
        .await?;

        if let Some(j) = job {
            if let Some(last_run) = j.last_run {
                vec![JobHistoryEntry {
                    job_name: j.job_name,
                    status: j.last_status.unwrap_or_else(|| "unknown".to_string()),
                    run_at: last_run,
                    error: j.last_error,
                }]
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    } else {
        let jobs = sqlx::query_as!(
            ScheduledJob,
            r#"
            SELECT id, job_name, schedule, enabled, last_run, next_run,
                   last_status, last_error, run_count, created_at, updated_at
            FROM scheduled_jobs
            WHERE last_run IS NOT NULL
            ORDER BY last_run DESC
            LIMIT $1
            "#,
            args.limit
        )
        .fetch_all(&*pool)
        .await?;

        jobs.into_iter()
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
