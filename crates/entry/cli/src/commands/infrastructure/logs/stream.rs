//! `infra logs stream` command tailing filtered log entries.
//!
//! Each poll's new entries are rendered through the host as they arrive; the
//! command's own output is the closing count of entries streamed and polls
//! made.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use clap::Args;
use std::time::Duration;
use systemprompt_logging::models::LogEntry;
use systemprompt_logging::{CliService, LogFilter, LogLevel, LoggingMaintenanceService};
use systemprompt_models::artifacts::NoticeLine;
use tokio::time;

use super::shared::level_notice;
use systemprompt_database::pool_from_handle;
use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Args)]
pub struct StreamArgs {
    #[arg(long, help = "Filter by log level (error, warn, info, debug, trace)")]
    pub level: Option<LogLevel>,

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

    #[arg(
        long,
        default_value = "0",
        help = "Stop after this many polling iterations (0 = run until interrupted)"
    )]
    pub max_iterations: u64,
}

pub(super) async fn execute(args: StreamArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    if host.output().is_json_output() {
        return Err(anyhow!("JSON output is not supported in streaming mode"));
    }

    let service = LoggingMaintenanceService::new(&pool_from_handle(host.db().await?)?);

    let mut last_timestamp: Option<DateTime<Utc>> = None;

    host.render(&header(&args));

    let mut iterations: u64 = 0;
    let mut streamed: usize = 0;
    loop {
        if args.clear {
            CliService::clear_screen();
            host.render(&header(&args));
        }

        let logs = get_logs(&service, &args, last_timestamp).await?;

        if !logs.is_empty() {
            host.render(&CommandOutput::message(
                logs.iter().map(entry_notice).collect(),
            ));
            streamed += logs.len();
            last_timestamp = logs.iter().map(|log| log.timestamp).max();
        } else if last_timestamp.is_none() {
            host.render(&CommandOutput::message(vec![NoticeLine::new(
                "warning",
                "No logs found. Waiting for new entries...",
            )]));
        }

        iterations += 1;
        if args.max_iterations != 0 && iterations >= args.max_iterations {
            return Ok(CommandOutput::message(vec![NoticeLine::new(
                "success",
                format!("Streamed {streamed} log entries over {iterations} polls"),
            )]));
        }

        time::sleep(Duration::from_millis(args.interval)).await;
    }
}

fn build_filter(args: &StreamArgs, since: Option<DateTime<Utc>>, limit: i32) -> LogFilter {
    let mut filter = LogFilter::new(1, limit);

    if let Some(level) = args.level {
        filter = filter.with_level(level.as_str());
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

    logs.sort_by_key(|x| x.timestamp);
    Ok(logs)
}

fn header(args: &StreamArgs) -> CommandOutput {
    let mut lines = Vec::new();
    if let Some(level) = args.level {
        lines.push(NoticeLine::new(
            "info",
            format!("Level filter: {}", level.as_str()),
        ));
    }
    if let Some(ref module) = args.module {
        lines.push(NoticeLine::new("info", format!("Module filter: {module}")));
    }
    lines.push(NoticeLine::new(
        "info",
        format!("Polling interval: {}ms", args.interval),
    ));
    CommandOutput::message(lines).with_title("Log Stream")
}

fn entry_notice(log: &LogEntry) -> NoticeLine {
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

    level_notice(log.level, line)
}
