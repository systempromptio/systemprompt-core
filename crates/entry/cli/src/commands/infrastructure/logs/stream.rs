use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use clap::Args;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_logging::models::{LogEntry, LogLevel};
use systemprompt_logging::{CliService, LoggingMaintenanceService};
use systemprompt_runtime::AppContext;
use tokio::time;

use crate::CliConfig;

#[derive(Debug, Args)]
pub struct StreamArgs {
    #[arg(long, help = "Filter by log level (error, warn, info, debug, trace)")]
    pub level: Option<String>,

    #[arg(long, help = "Filter by module name (partial match)")]
    pub module: Option<String>,

    #[arg(
        long,
        default_value = "1000",
        help = "Polling interval in milliseconds"
    )]
    pub interval: u64,

    #[arg(long, help = "Clear screen between updates")]
    pub clear: bool,
}

pub async fn execute(args: StreamArgs, config: &CliConfig) -> Result<()> {
    if config.is_json_output() {
        return Err(anyhow!("JSON output is not supported in streaming mode"));
    }

    let ctx = AppContext::new().await?;
    let service = LoggingMaintenanceService::new(Arc::clone(ctx.db_pool()));

    let mut last_timestamp: Option<DateTime<Utc>> = None;

    CliService::section("Log Stream");
    display_filters(&args);

    loop {
        if args.clear {
            CliService::clear_screen();
            CliService::section("Log Stream");
        }

        let logs = get_logs(&service, &args, last_timestamp).await?;

        if !logs.is_empty() {
            for log in &logs {
                display_log_entry(log);
            }
            last_timestamp = logs.iter().map(|log| log.timestamp).max();
        } else if last_timestamp.is_none() {
            CliService::warning("No logs found. Waiting for new entries...");
        }

        time::sleep(Duration::from_millis(args.interval)).await;
    }
}

async fn get_logs(
    service: &LoggingMaintenanceService,
    args: &StreamArgs,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<LogEntry>> {
    match since {
        None => get_initial_logs(service, args).await,
        Some(ts) => get_new_logs(service, args, ts).await,
    }
}

async fn get_initial_logs(
    service: &LoggingMaintenanceService,
    args: &StreamArgs,
) -> Result<Vec<LogEntry>> {
    let mut logs = service
        .get_recent_logs(20)
        .await
        .map_err(|e| anyhow!("Failed to get initial logs: {}", e))?;

    apply_filters(&mut logs, args);
    Ok(logs)
}

async fn get_new_logs(
    service: &LoggingMaintenanceService,
    args: &StreamArgs,
    since: DateTime<Utc>,
) -> Result<Vec<LogEntry>> {
    let all_recent_logs = service
        .get_recent_logs(100)
        .await
        .map_err(|e| anyhow!("Failed to get recent logs: {}", e))?;

    let mut new_logs: Vec<LogEntry> = all_recent_logs
        .into_iter()
        .filter(|log| log.timestamp > since)
        .collect();

    apply_filters(&mut new_logs, args);
    new_logs.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    Ok(new_logs)
}

fn apply_filters(logs: &mut Vec<LogEntry>, args: &StreamArgs) {
    if let Some(ref level_str) = args.level {
        if let Ok(level) = level_str.parse::<LogLevel>() {
            logs.retain(|log| log.level == level);
        }
    }

    if let Some(ref module) = args.module {
        logs.retain(|log| log.module.contains(module));
    }
}

fn display_filters(args: &StreamArgs) {
    if let Some(ref level) = args.level {
        CliService::key_value("Level filter", level);
    }
    if let Some(ref module) = args.module {
        CliService::key_value("Module filter", module);
    }
    CliService::key_value("Polling interval", &format!("{}ms", args.interval));
}

fn display_log_entry(log: &LogEntry) {
    let timestamp = log.timestamp.format("%H:%M:%S%.3f");
    let level_str = match log.level {
        LogLevel::Error => "ERROR",
        LogLevel::Warn => "WARN ",
        LogLevel::Info => "INFO ",
        LogLevel::Debug => "DEBUG",
        LogLevel::Trace => "TRACE",
    };

    let line = log.metadata.as_ref().map_or_else(
        || {
            format!(
                "{} {} [{}] {}",
                timestamp, level_str, log.module, log.message
            )
        },
        |metadata| {
            let metadata_str = serde_json::to_string(metadata).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to serialize log metadata");
                String::new()
            });
            format!(
                "{} {} [{}] {} {}",
                timestamp, level_str, log.module, log.message, metadata_str
            )
        },
    );

    match log.level {
        LogLevel::Error => CliService::error(&line),
        LogLevel::Warn => CliService::warning(&line),
        _ => CliService::info(&line),
    }
}
