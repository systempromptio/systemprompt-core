#![allow(clippy::print_stdout)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Args, Subcommand};
use std::sync::Arc;
use std::time::Duration;
use systemprompt_core_logging::models::{LogEntry, LogLevel};
use systemprompt_core_logging::{CliService, LoggingMaintenanceService};
use systemprompt_runtime::AppContext;
use tokio::time;

#[derive(Subcommand)]
pub enum LogCommands {
    View(ViewArgs),
    ClearAll(MaintenanceArgs),
    Cleanup(CleanupArgs),
}

#[derive(Args)]
pub struct ViewArgs {
    #[arg(long)]
    level: Option<String>,

    #[arg(long)]
    module: Option<String>,

    #[arg(long, default_value = "20")]
    tail: i64,

    #[arg(long, short = 's')]
    stream: bool,

    #[arg(long, default_value = "1000")]
    interval: u64,

    #[arg(long)]
    clear_screen: bool,
}

#[derive(Args)]
pub struct MaintenanceArgs {
    #[arg(long)]
    vacuum: bool,
}

#[derive(Args)]
pub struct CleanupArgs {
    #[arg(long)]
    older_than: Option<i64>,

    #[arg(long)]
    keep_last_days: Option<i64>,

    #[arg(long)]
    level: Option<String>,

    #[arg(long)]
    module: Option<String>,

    #[arg(long)]
    dry_run: bool,

    #[arg(long)]
    vacuum: bool,
}

async fn get_initial_logs(
    service: &LoggingMaintenanceService,
    args: &ViewArgs,
) -> Result<Vec<LogEntry>> {
    if args.level.is_some() || args.module.is_some() {
        CliService::warning(
            "Filtering by level/module not yet implemented in refactored Log module",
        );
    }
    service
        .get_recent_logs(args.tail)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get initial logs: {}", e))
}

async fn get_new_logs(
    service: &LoggingMaintenanceService,
    args: &ViewArgs,
    since: DateTime<Utc>,
) -> Result<Vec<LogEntry>> {
    let all_recent_logs = service
        .get_recent_logs(100)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get recent logs: {}", e))?;

    let mut new_logs: Vec<LogEntry> = all_recent_logs
        .into_iter()
        .filter(|log| log.timestamp > since)
        .collect();

    if let Some(ref level_str) = args.level {
        let level: LogLevel = level_str
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid log level: {}", level_str))?;
        new_logs.retain(|log| log.level == level);
    }

    if let Some(ref module) = args.module {
        new_logs.retain(|log| log.module.contains(module));
    }

    new_logs.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    Ok(new_logs)
}

fn format_log(log: &LogEntry) -> String {
    let timestamp = log.timestamp.format("%H:%M:%S%.3f");
    let level_str = match log.level {
        LogLevel::Error => "ERROR",
        LogLevel::Warn => "WARN ",
        LogLevel::Info => "INFO ",
        LogLevel::Debug => "DEBUG",
        LogLevel::Trace => "TRACE",
    };

    if let Some(ref metadata) = log.metadata {
        format!(
            "{} {} [{}] {} {}",
            timestamp,
            level_str,
            log.module,
            log.message,
            serde_json::to_string(metadata).unwrap_or_default()
        )
    } else {
        format!(
            "{} {} [{}] {}",
            timestamp, level_str, log.module, log.message
        )
    }
}

async fn execute_cleanup(service: &LoggingMaintenanceService, args: &CleanupArgs) -> Result<()> {
    let days = args.older_than.or(args.keep_last_days).ok_or_else(|| {
        anyhow::anyhow!("Please specify --older-than <DAYS> or --keep-last-days <DAYS>")
    })?;

    let cutoff = Utc::now() - chrono::Duration::days(days);

    CliService::section("Log Cleanup");

    if let Some(ref level) = args.level {
        CliService::key_value("Level", level);
    }
    if let Some(ref module) = args.module {
        CliService::key_value("Module", module);
    }
    CliService::key_value(
        "Cutoff",
        &format!("{} ({} days ago)", cutoff.format("%Y-%m-%d %H:%M:%S"), days),
    );

    if args.dry_run {
        CliService::warning("DRY RUN MODE - No logs will be deleted");
    }

    let deleted = if args.dry_run {
        service
            .get_recent_logs(1000)
            .await?
            .iter()
            .filter(|log| log.timestamp < cutoff)
            .count() as u64
    } else {
        service.cleanup_old_logs(cutoff).await?
    };

    CliService::section("Results");
    CliService::key_value("Logs to be deleted", &deleted.to_string());

    if args.dry_run {
        CliService::info("Run without --dry-run to actually delete logs");
    } else {
        CliService::success("Cleanup complete!");

        if args.vacuum {
            CliService::info("Running VACUUM to reclaim disk space...");
            LoggingMaintenanceService::vacuum();
            CliService::success("VACUUM complete");
        }
    }

    Ok(())
}

async fn execute_view(service: &LoggingMaintenanceService, args: &ViewArgs) -> Result<()> {
    let mut last_timestamp: Option<DateTime<Utc>> = None;

    CliService::section("SystemPrompt Log Stream");

    if let Some(ref level) = args.level {
        CliService::key_value("Filtering by level", level);
    }
    if let Some(ref module) = args.module {
        CliService::key_value("Filtering by module", module);
    }
    if args.stream {
        CliService::key_value(
            "Streaming mode",
            &format!("enabled (refresh interval: {}ms)", args.interval),
        );
    }

    loop {
        if args.clear_screen {
            print!("\x1B[2J\x1B[1;1H");
            CliService::section("SystemPrompt Log Stream");
        }

        let logs = match last_timestamp {
            None => get_initial_logs(service, args).await?,
            Some(ts) => get_new_logs(service, args, ts).await?,
        };

        if !logs.is_empty() {
            for log in &logs {
                CliService::info(&format_log(log));
            }
            last_timestamp = logs.iter().map(|log| log.timestamp).max();
        } else if last_timestamp.is_none() {
            CliService::warning("No logs found");
        }

        if !args.stream {
            break;
        }

        time::sleep(Duration::from_millis(args.interval)).await;
    }

    Ok(())
}

async fn execute_clear_all(
    service: &LoggingMaintenanceService,
    args: &MaintenanceArgs,
) -> Result<()> {
    let cleared = service.clear_all_logs().await?;
    CliService::success(&format!("Cleared {} log entries", cleared));

    if args.vacuum {
        CliService::info("Running VACUUM to reclaim disk space...");
        LoggingMaintenanceService::vacuum();
        CliService::success("VACUUM complete");
    }

    Ok(())
}

pub async fn execute(cmd: Option<LogCommands>) -> Result<()> {
    let ctx = AppContext::new().await?;
    let service = LoggingMaintenanceService::new(Arc::clone(ctx.db_pool()));

    match cmd {
        Some(LogCommands::View(args)) => execute_view(&service, &args).await,
        Some(LogCommands::ClearAll(args)) => execute_clear_all(&service, &args).await,
        Some(LogCommands::Cleanup(args)) => execute_cleanup(&service, &args).await,
        None => {
            execute_view(
                &service,
                &ViewArgs {
                    level: None,
                    module: None,
                    tail: 20,
                    stream: false,
                    interval: 1000,
                    clear_screen: false,
                },
            )
            .await
        },
    }
}
