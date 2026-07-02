use anyhow::{Result, anyhow};
use chrono::Utc;
use clap::Args;
use systemprompt_logging::LoggingMaintenanceService;

use super::LogCleanupOutput;
use super::duration::parse_duration;
use crate::context::CommandContext;
use crate::interactive::require_confirmation;
use crate::shared::{CommandOutput, render_result};

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

pub(super) async fn execute(args: CleanupArgs, ctx: &CommandContext) -> Result<()> {
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

    let service = LoggingMaintenanceService::new(&ctx.db_pool().await?)?;

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

        let result = CommandOutput::card_value("Cleanup Preview (Dry Run)", &output);
        render_result(&result, &ctx.cli);
        return Ok(());
    }

    require_confirmation(
        ctx.prompter(),
        &format!(
            "Delete logs older than {}? This cannot be undone.",
            cutoff_str
        ),
        args.yes,
        &ctx.cli,
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

    let result = CommandOutput::card_value("Logs Cleaned Up", &output);
    render_result(&result, &ctx.cli);

    Ok(())
}
