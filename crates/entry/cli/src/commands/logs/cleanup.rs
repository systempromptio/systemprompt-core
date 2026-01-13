use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use clap::Args;
use std::sync::Arc;
use systemprompt_core_logging::{CliService, LoggingMaintenanceService};
use systemprompt_runtime::AppContext;

use super::LogCleanupOutput;
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

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim().to_lowercase();

    if let Some(days) = s.strip_suffix('d') {
        let num: i64 = days
            .parse()
            .map_err(|_| anyhow!("Invalid duration: {}", s))?;
        return Ok(Duration::days(num));
    }

    if let Some(hours) = s.strip_suffix('h') {
        let num: i64 = hours
            .parse()
            .map_err(|_| anyhow!("Invalid duration: {}", s))?;
        return Ok(Duration::hours(num));
    }

    if let Some(mins) = s.strip_suffix('m') {
        let num: i64 = mins
            .parse()
            .map_err(|_| anyhow!("Invalid duration: {}", s))?;
        return Ok(Duration::minutes(num));
    }

    // Try parsing as plain number of days
    if let Ok(num) = s.parse::<i64>() {
        return Ok(Duration::days(num));
    }

    Err(anyhow!(
        "Invalid duration format: {}. Use formats like '7d', '24h', '30m'",
        s
    ))
}

pub async fn execute(args: CleanupArgs, config: &CliConfig) -> Result<()> {
    // Require either --older-than or --keep-last-days
    let cutoff_duration = match (&args.older_than, args.keep_last_days) {
        (Some(duration_str), None) => parse_duration(duration_str)?,
        (None, Some(days)) => Duration::days(days),
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

    // Require --yes for destructive operation
    if !args.yes {
        if config.is_interactive() {
            let msg = format!(
                "Delete logs older than {}? This cannot be undone.",
                cutoff_str
            );
            if !CliService::confirm(&msg)? {
                CliService::info("Cancelled");
                return Ok(());
            }
        } else {
            return Err(anyhow!("--yes is required in non-interactive mode"));
        }
    }

    let ctx = AppContext::new().await?;
    let service = LoggingMaintenanceService::new(Arc::clone(ctx.db_pool()));

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
