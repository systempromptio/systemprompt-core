use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use clap::Args;
use std::time::Duration;
use systemprompt_logging::models::LogEntry;
use systemprompt_logging::{CliService, LogFilter, LogLevel, LoggingMaintenanceService};
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
    let service = LoggingMaintenanceService::new(ctx.db_pool())?;

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

fn build_filter(args: &StreamArgs, since: Option<DateTime<Utc>>, limit: i32) -> LogFilter {
    let mut filter = LogFilter::new(1, limit);

    if let Some(ref level) = args.level {
        filter = filter.with_level(level.to_uppercase());
    }
    if let Some(ref module) = args.module {
        filter = filter.with_module(module);
    }
    if let Some(since) = since {
        filter = filter.with_since(since);
    }

    filter
}

async fn get_logs(
    service: &LoggingMaintenanceService,
    args: &StreamArgs,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<LogEntry>> {
    let limit = if since.is_some() { 100 } else { 20 };
    let filter = build_filter(args, since, limit);

    let (mut logs, _count) = service
        .get_filtered_logs(&filter)
        .await
        .map_err(|e| anyhow!("Failed to get logs: {}", e))?;

    logs.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    Ok(logs)
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
