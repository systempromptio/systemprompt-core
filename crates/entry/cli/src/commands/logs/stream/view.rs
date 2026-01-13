use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_core_logging::models::{LogEntry, LogLevel};
use systemprompt_core_logging::{CliService, LoggingMaintenanceService};
use systemprompt_runtime::AppContext;
use tokio::time;

use super::{LogEntryRow, LogFilters, LogViewOutput};
use crate::shared::{render_result, CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Args)]
pub struct ViewArgs {
    #[arg(long, help = "Filter by log level (error, warn, info, debug, trace)")]
    pub level: Option<String>,

    #[arg(long, help = "Filter by module name (partial match)")]
    pub module: Option<String>,

    #[arg(long, default_value = "20", help = "Number of log entries to show")]
    pub tail: i64,

    #[arg(long, short = 's', help = "Stream logs in real-time")]
    pub stream: bool,

    #[arg(long, default_value = "1000", help = "Polling interval in milliseconds")]
    pub interval: u64,

    #[arg(long, help = "Clear screen between updates (streaming mode)")]
    pub clear_screen: bool,
}

pub async fn execute(args: ViewArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let service = LoggingMaintenanceService::new(Arc::clone(ctx.db_pool()));

    if args.stream {
        execute_streaming(&service, &args, config).await
    } else {
        execute_single(&service, &args, config).await
    }
}

async fn execute_single(
    service: &LoggingMaintenanceService,
    args: &ViewArgs,
    config: &CliConfig,
) -> Result<()> {
    let logs = get_logs(service, args, None).await?;
    let output = build_output(&logs, args);

    if config.is_json_output() {
        let hints = RenderingHints {
            columns: Some(vec![
                "timestamp".to_string(),
                "level".to_string(),
                "module".to_string(),
                "message".to_string(),
            ]),
            ..Default::default()
        };
        let result = CommandResult::table(output)
            .with_title("Log Entries")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_logs(&output);
    }

    Ok(())
}

async fn execute_streaming(
    service: &LoggingMaintenanceService,
    args: &ViewArgs,
    config: &CliConfig,
) -> Result<()> {
    if config.is_json_output() {
        return Err(anyhow::anyhow!(
            "JSON output is not supported in streaming mode"
        ));
    }

    let mut last_timestamp: Option<DateTime<Utc>> = None;

    CliService::section("SystemPrompt Log Stream");
    display_filters(args);

    loop {
        if args.clear_screen {
            CliService::clear_screen();
            CliService::section("SystemPrompt Log Stream");
        }

        let logs = get_logs(service, args, last_timestamp).await?;

        if !logs.is_empty() {
            for log in &logs {
                display_log_entry(log);
            }
            last_timestamp = logs.iter().map(|log| log.timestamp).max();
        } else if last_timestamp.is_none() {
            CliService::warning("No logs found");
        }

        time::sleep(Duration::from_millis(args.interval)).await;
    }
}

async fn get_logs(
    service: &LoggingMaintenanceService,
    args: &ViewArgs,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<LogEntry>> {
    match since {
        None => get_initial_logs(service, args).await,
        Some(ts) => get_new_logs(service, args, ts).await,
    }
}

async fn get_initial_logs(
    service: &LoggingMaintenanceService,
    args: &ViewArgs,
) -> Result<Vec<LogEntry>> {
    let mut logs = service
        .get_recent_logs(args.tail)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get initial logs: {}", e))?;

    apply_filters(&mut logs, args);
    Ok(logs)
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

    apply_filters(&mut new_logs, args);
    new_logs.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    Ok(new_logs)
}

fn apply_filters(logs: &mut Vec<LogEntry>, args: &ViewArgs) {
    if let Some(ref level_str) = args.level {
        if let Ok(level) = level_str.parse::<LogLevel>() {
            logs.retain(|log| log.level == level);
        }
    }

    if let Some(ref module) = args.module {
        logs.retain(|log| log.module.contains(module));
    }
}

fn build_output(logs: &[LogEntry], args: &ViewArgs) -> LogViewOutput {
    let log_rows: Vec<LogEntryRow> = logs.iter().map(log_to_row).collect();

    LogViewOutput {
        total: log_rows.len() as u64,
        logs: log_rows,
        filters: LogFilters {
            level: args.level.clone(),
            module: args.module.clone(),
            tail: args.tail,
        },
    }
}

fn log_to_row(log: &LogEntry) -> LogEntryRow {
    LogEntryRow {
        timestamp: log.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
        level: format!("{:?}", log.level).to_uppercase(),
        module: log.module.clone(),
        message: log.message.clone(),
        metadata: log.metadata.clone(),
    }
}

fn render_logs(output: &LogViewOutput) {
    CliService::section("SystemPrompt Log Stream");

    if output.filters.level.is_some() || output.filters.module.is_some() {
        if let Some(ref level) = output.filters.level {
            CliService::key_value("Filtering by level", level);
        }
        if let Some(ref module) = output.filters.module {
            CliService::key_value("Filtering by module", module);
        }
    }

    if output.logs.is_empty() {
        CliService::warning("No logs found");
        return;
    }

    for log in &output.logs {
        display_log_row(log);
    }

    CliService::info(&format!("Showing {} log entries", output.total));
}

fn display_filters(args: &ViewArgs) {
    if let Some(ref level) = args.level {
        CliService::key_value("Filtering by level", level);
    }
    if let Some(ref module) = args.module {
        CliService::key_value("Filtering by module", module);
    }
    CliService::key_value(
        "Streaming mode",
        &format!("enabled (refresh interval: {}ms)", args.interval),
    );
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
        || format!("{} {} [{}] {}", timestamp, level_str, log.module, log.message),
        |metadata| {
            format!(
                "{} {} [{}] {} {}",
                timestamp,
                level_str,
                log.module,
                log.message,
                serde_json::to_string(metadata).unwrap_or_default()
            )
        },
    );

    match log.level {
        LogLevel::Error => CliService::error(&line),
        LogLevel::Warn => CliService::warning(&line),
        _ => CliService::info(&line),
    }
}

fn display_log_row(log: &LogEntryRow) {
    let time_part = if log.timestamp.len() >= 23 {
        &log.timestamp[11..23]
    } else {
        &log.timestamp
    };

    let line = format!("{} {} [{}] {}", time_part, log.level, log.module, log.message);

    match log.level.as_str() {
        "ERROR" => CliService::error(&line),
        "WARN" => CliService::warning(&line),
        _ => CliService::info(&line),
    }
}
