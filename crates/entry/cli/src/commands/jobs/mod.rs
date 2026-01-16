pub mod types;

use crate::cli_settings::CliConfig;
use crate::shared::{render_result, CommandResult, RenderingHints};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::sync::Arc;
use std::time::Instant;
use systemprompt_core_analytics::SessionCleanupService;
use systemprompt_core_scheduler::{JobRepository, ScheduledJob};
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext};
use types::{
    BatchJobRunOutput, JobEnableOutput, JobHistoryEntry, JobHistoryOutput, JobInfo, JobListOutput,
    JobRunOutput, JobRunResult, JobShowOutput, LogCleanupOutput, SessionCleanupOutput,
};

use systemprompt_generator as _;

#[derive(Debug, Subcommand)]
pub enum JobsCommands {
    #[command(about = "List available jobs")]
    List,

    #[command(about = "Show detailed information about a job")]
    Show(ShowArgs),

    #[command(about = "Run a scheduled job manually")]
    Run(RunArgs),

    #[command(about = "View job execution history")]
    History(HistoryArgs),

    #[command(about = "Enable a job")]
    Enable(EnableArgs),

    #[command(about = "Disable a job")]
    Disable(DisableArgs),

    #[command(about = "Clean up inactive sessions")]
    CleanupSessions(CleanupSessionsArgs),

    #[command(about = "Clean up old log entries")]
    LogCleanup(LogCleanupArgs),

    #[command(about = "Clean up inactive sessions (alias)", hide = true)]
    SessionCleanup(CleanupSessionsArgs),
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Job name to show details for")]
    pub job_name: String,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(help = "Job name(s) to run", num_args = 1..)]
    pub job_names: Vec<String>,

    #[arg(long, help = "Run all enabled jobs")]
    pub all: bool,

    #[arg(long, help = "Run jobs sequentially instead of in parallel")]
    pub sequential: bool,
}

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

#[derive(Debug, Args)]
pub struct EnableArgs {
    #[arg(help = "Job name to enable")]
    pub job_name: String,
}

#[derive(Debug, Args)]
pub struct DisableArgs {
    #[arg(help = "Job name to disable")]
    pub job_name: String,
}

#[derive(Debug, Clone, Copy, Args)]
pub struct CleanupSessionsArgs {
    #[arg(
        long,
        default_value = "1",
        help = "Sessions inactive for more than N hours"
    )]
    pub hours: i32,

    #[arg(long, help = "Preview what would be cleaned without executing")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, Args)]
pub struct LogCleanupArgs {
    #[arg(long, default_value = "30", help = "Delete logs older than N days")]
    pub days: i32,

    #[arg(long, help = "Preview what would be cleaned without executing")]
    pub dry_run: bool,
}

pub async fn execute(cmd: JobsCommands, _config: &CliConfig) -> Result<()> {
    match cmd {
        JobsCommands::List => {
            let result = list_jobs()?;
            render_result(&result);
            Ok(())
        },
        JobsCommands::Show(args) => {
            let result = show_job(args).await?;
            render_result(&result);
            Ok(())
        },
        JobsCommands::Run(args) => {
            let result = run_jobs(args).await?;
            render_result(&result);
            Ok(())
        },
        JobsCommands::History(args) => {
            let result = job_history(args).await?;
            render_result(&result);
            Ok(())
        },
        JobsCommands::Enable(args) => {
            let result = enable_job(args).await?;
            render_result(&result);
            Ok(())
        },
        JobsCommands::Disable(args) => {
            let result = disable_job(args).await?;
            render_result(&result);
            Ok(())
        },
        JobsCommands::CleanupSessions(args) | JobsCommands::SessionCleanup(args) => {
            let result = cleanup_sessions(args).await?;
            render_result(&result);
            Ok(())
        },
        JobsCommands::LogCleanup(args) => {
            let result = cleanup_logs(args).await?;
            render_result(&result);
            Ok(())
        },
    }
}

fn list_jobs() -> Result<CommandResult<JobListOutput>> {
    let jobs: Vec<JobInfo> = inventory::iter::<&'static dyn Job>
        .into_iter()
        .map(|job| JobInfo {
            name: job.name().to_string(),
            description: job.description().to_string(),
            schedule: job.schedule().to_string(),
            enabled: job.enabled(),
        })
        .collect();

    let total = jobs.len();
    let output = JobListOutput { jobs, total };

    Ok(CommandResult::table(output)
        .with_title("Available Jobs")
        .with_hints(RenderingHints {
            columns: Some(vec![
                "name".to_string(),
                "description".to_string(),
                "schedule".to_string(),
                "enabled".to_string(),
            ]),
            ..Default::default()
        }))
}

async fn show_job(args: ShowArgs) -> Result<CommandResult<JobShowOutput>> {
    let job = inventory::iter::<&'static dyn Job>
        .into_iter()
        .find(|&j| j.name() == args.job_name)
        .copied();

    let Some(job) = job else {
        anyhow::bail!(
            "Unknown job: {}. Use 'jobs list' to see available jobs",
            args.job_name
        );
    };

    let ctx = Arc::new(AppContext::new().await?);
    let repo = JobRepository::new(ctx.db_pool())?;

    let db_job: Option<ScheduledJob> = repo.find_job(&args.job_name).await?;

    let output = JobShowOutput {
        name: job.name().to_string(),
        description: job.description().to_string(),
        schedule: job.schedule().to_string(),
        schedule_human: parse_cron_human(job.schedule()),
        enabled: db_job.as_ref().map_or(job.enabled(), |j| j.enabled),
        last_run: db_job.as_ref().and_then(|j| j.last_run),
        next_run: db_job.as_ref().and_then(|j| j.next_run),
        last_status: db_job.as_ref().and_then(|j| j.last_status.clone()),
        last_error: db_job.as_ref().and_then(|j| j.last_error.clone()),
        run_count: db_job.as_ref().map_or(0, |j| j.run_count),
    };

    Ok(CommandResult::card(output).with_title(format!("Job: {}", args.job_name)))
}

async fn run_jobs(args: RunArgs) -> Result<CommandResult<BatchJobRunOutput>> {
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

async fn job_history(args: HistoryArgs) -> Result<CommandResult<JobHistoryOutput>> {
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

async fn enable_job(args: EnableArgs) -> Result<CommandResult<JobEnableOutput>> {
    let job = inventory::iter::<&'static dyn Job>
        .into_iter()
        .find(|&j| j.name() == args.job_name)
        .copied();

    if job.is_none() {
        anyhow::bail!(
            "Unknown job: {}. Use 'jobs list' to see available jobs",
            args.job_name
        );
    }

    let ctx = Arc::new(AppContext::new().await?);
    let pool = ctx.db_pool().pool_arc()?;

    sqlx::query!(
        "UPDATE scheduled_jobs SET enabled = true, updated_at = NOW() WHERE job_name = $1",
        args.job_name
    )
    .execute(&*pool)
    .await
    .context("Failed to enable job")?;

    let output = JobEnableOutput {
        job_name: args.job_name.clone(),
        enabled: true,
        message: format!("Job '{}' has been enabled", args.job_name),
    };

    Ok(CommandResult::text(output).with_title("Job Enabled"))
}

async fn disable_job(args: DisableArgs) -> Result<CommandResult<JobEnableOutput>> {
    let job = inventory::iter::<&'static dyn Job>
        .into_iter()
        .find(|&j| j.name() == args.job_name)
        .copied();

    if job.is_none() {
        anyhow::bail!(
            "Unknown job: {}. Use 'jobs list' to see available jobs",
            args.job_name
        );
    }

    let ctx = Arc::new(AppContext::new().await?);
    let pool = ctx.db_pool().pool_arc()?;

    sqlx::query!(
        "UPDATE scheduled_jobs SET enabled = false, updated_at = NOW() WHERE job_name = $1",
        args.job_name
    )
    .execute(&*pool)
    .await
    .context("Failed to disable job")?;

    let output = JobEnableOutput {
        job_name: args.job_name.clone(),
        enabled: false,
        message: format!("Job '{}' has been disabled", args.job_name),
    };

    Ok(CommandResult::text(output).with_title("Job Disabled"))
}

async fn cleanup_sessions(
    args: CleanupSessionsArgs,
) -> Result<CommandResult<SessionCleanupOutput>> {
    let ctx = Arc::new(AppContext::new().await?);

    if args.dry_run {
        let pool = ctx.db_pool().pool_arc()?;
        let cutoff_hours = args.hours;

        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM user_sessions
            WHERE ended_at IS NULL
              AND last_activity_at < NOW() - ($1 || ' hours')::INTERVAL
            "#,
            cutoff_hours.to_string()
        )
        .fetch_one(&*pool)
        .await?;

        let output = SessionCleanupOutput {
            job_name: "session_cleanup".to_string(),
            sessions_cleaned: 0,
            hours_threshold: args.hours,
            message: format!(
                "DRY RUN: Would clean up {} inactive session(s) older than {} hour(s)",
                count, args.hours
            ),
        };

        return Ok(CommandResult::text(output).with_title("Session Cleanup (Dry Run)"));
    }

    let cleanup_service = SessionCleanupService::new(Arc::clone(ctx.db_pool()));
    let closed_count = cleanup_service
        .cleanup_inactive_sessions(args.hours)
        .await?;

    let output = SessionCleanupOutput {
        job_name: "session_cleanup".to_string(),
        sessions_cleaned: closed_count as i64,
        hours_threshold: args.hours,
        message: format!("Cleaned up {} inactive session(s)", closed_count),
    };

    Ok(CommandResult::text(output).with_title("Session Cleanup"))
}

async fn cleanup_logs(args: LogCleanupArgs) -> Result<CommandResult<LogCleanupOutput>> {
    let ctx = Arc::new(AppContext::new().await?);
    let pool = ctx.db_pool().pool_arc()?;

    if args.dry_run {
        let count: i64 = sqlx::query_scalar::<_, i64>(
            r"
            SELECT COUNT(*)
            FROM application_logs
            WHERE created_at < NOW() - ($1 || ' days')::INTERVAL
            ",
        )
        .bind(args.days.to_string())
        .fetch_one(&*pool)
        .await
        .unwrap_or(0);

        let output = LogCleanupOutput {
            job_name: "log_cleanup".to_string(),
            entries_deleted: 0,
            days_threshold: args.days,
            message: format!(
                "DRY RUN: Would delete {} log entries older than {} day(s)",
                count, args.days
            ),
        };

        return Ok(CommandResult::text(output).with_title("Log Cleanup (Dry Run)"));
    }

    let deleted_count = sqlx::query(
        r"
        DELETE FROM application_logs
        WHERE created_at < NOW() - ($1 || ' days')::INTERVAL
        ",
    )
    .bind(args.days.to_string())
    .execute(&*pool)
    .await?
    .rows_affected() as i64;

    let output = LogCleanupOutput {
        job_name: "log_cleanup".to_string(),
        entries_deleted: deleted_count,
        days_threshold: args.days,
        message: format!(
            "Deleted {} log entries older than {} day(s)",
            deleted_count, args.days
        ),
    };

    Ok(CommandResult::text(output).with_title("Log Cleanup"))
}

fn parse_cron_human(schedule: &str) -> String {
    let parts: Vec<&str> = schedule.split_whitespace().collect();
    if parts.len() != 6 {
        return schedule.to_string();
    }

    match (parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]) {
        ("0", "0", "*", "*", "*", "*") => "Every hour".to_string(),
        ("0", min, "*", "*", "*", "*") if min.starts_with("*/") => {
            format!("Every {} minutes", &min[2..])
        },
        ("0", "0", hour, "*", "*", "*") if hour.starts_with("*/") => {
            format!("Every {} hours", &hour[2..])
        },
        ("0", "0", hour, "*", "*", "*") => format!("Daily at {}:00", hour),
        ("0", min, hour, "*", "*", "*") => format!("Daily at {}:{}", hour, min),
        ("*", "*", "*", "*", "*", "*") => "Every second".to_string(),
        _ => schedule.to_string(),
    }
}
