use anyhow::{anyhow, Result};
use chrono::Utc;
use clap::Args;
use systemprompt_logging::LoggingMaintenanceService;
use systemprompt_runtime::AppContext;

use super::duration::parse_duration;
use super::LogCleanupOutput;
use crate::interactive::require_confirmation;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct CleanupArgs {
    #[arg(
        long,
        help = "Delete logs older than this duration (e.g., '7d', '24h', '30d')",
        conflicts_with = "keep_last_days"
    )]
    pub older_than: Option<String>,

    #[arg(
        long,
        help = "Keep logs from the last N days",
        conflicts_with = "older_than"
    )]
    pub keep_last_days: Option<i64>,

    #[arg(long, help = "Preview what would be deleted without making changes")]
    pub dry_run: bool,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

pub async fn execute(args: CleanupArgs, config: &CliConfig) -> Result<()> {
    let cutoff_duration = match (&args.older_than, args.keep_last_days) {
        (Some(duration_str), None) => parse_duration(duration_str)?,
        (None, Some(days)) => chrono::Duration::days(days),
        (None, None) => {
            return Err(anyhow!(
                "Either --older-than or --keep-last-days is required"
            ));
        },
        (Some(_), Some(_)) => {
            return Err(anyhow!(
                "--older-than and --keep-last-days are mutually exclusive"
            ));
        },
    };

    let cutoff_date = Utc::now() - cutoff_duration;
    let cutoff_str = cutoff_date.format("%Y-%m-%d %H:%M:%S UTC").to_string();

    let ctx = AppContext::new().await?;
    let service = LoggingMaintenanceService::new(ctx.db_pool())?;

    if args.dry_run {
        let count = service
            .count_logs_before(cutoff_date)
            .await
            .map_err(|e| anyhow!("Failed to count logs: {}", e))?;

        let output = LogCleanupOutput {
            deleted_count: count,
            dry_run: true,
            cutoff_date: cutoff_str,
            vacuum_performed: false,
        };

        let result = CommandResult::card(output).with_title("Cleanup Preview (Dry Run)");
        render_result(&result);
        return Ok(());
    }

    require_confirmation(
        &format!(
            "Delete logs older than {}? This cannot be undone.",
            cutoff_str
        ),
        args.yes,
        config,
    )?;

    let deleted_count = service
        .cleanup_old_logs(cutoff_date)
        .await
        .map_err(|e| anyhow!("Failed to cleanup logs: {}", e))?;

    let output = LogCleanupOutput {
        deleted_count,
        dry_run: false,
        cutoff_date: cutoff_str,
        vacuum_performed: false,
    };

    let result = CommandResult::card(output).with_title("Logs Cleaned Up");
    render_result(&result);

    Ok(())
}
