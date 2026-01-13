use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use clap::Args;
use std::sync::Arc;
use systemprompt_core_logging::models::{LogEntry, LogLevel};
use systemprompt_core_logging::{CliService, LoggingMaintenanceService};
use systemprompt_runtime::AppContext;

use super::duration::parse_since;
use super::{LogEntryRow, LogFilters, LogViewOutput};
use crate::shared::{render_result, CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ViewArgs {
    #[arg(long, help = "Filter by log level (error, warn, info, debug, trace)")]
    pub level: Option<String>,

    #[arg(long, help = "Filter by module name (partial match)")]
    pub module: Option<String>,

    #[arg(
        long,
        short = 'n',
        alias = "limit",
        default_value = "20",
        help = "Number of log entries to show"
    )]
    pub tail: i64,

    #[arg(
        long,
        help = "Only show logs since this duration (e.g., '1h', '24h', '7d') or datetime"
    )]
    pub since: Option<String>,
}

pub async fn execute(args: ViewArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let service = LoggingMaintenanceService::new(Arc::clone(ctx.db_pool()));

    let since_timestamp = parse_since(&args.since)?;
    let logs = get_logs(&service, &args, since_timestamp).await?;
    let output = build_output(&logs, &args);

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

async fn get_logs(
    service: &LoggingMaintenanceService,
    args: &ViewArgs,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<LogEntry>> {
    let mut logs = service
        .get_recent_logs(args.tail)
        .await
        .map_err(|e| anyhow!("Failed to get logs: {}", e))?;

    apply_filters(&mut logs, args, since);
    Ok(logs)
}

fn apply_filters(logs: &mut Vec<LogEntry>, args: &ViewArgs, since: Option<DateTime<Utc>>) {
    if let Some(ref level_str) = args.level {
        if let Ok(level) = level_str.parse::<LogLevel>() {
            logs.retain(|log| log.level == level);
        }
    }

    if let Some(ref module) = args.module {
        logs.retain(|log| log.module.contains(module));
    }

    if let Some(since_ts) = since {
        logs.retain(|log| log.timestamp >= since_ts);
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
            since: args.since.clone(),
            pattern: None,
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
    CliService::section("Log Entries");

    if output.filters.level.is_some()
        || output.filters.module.is_some()
        || output.filters.since.is_some()
    {
        if let Some(ref level) = output.filters.level {
            CliService::key_value("Level", level);
        }
        if let Some(ref module) = output.filters.module {
            CliService::key_value("Module", module);
        }
        if let Some(ref since) = output.filters.since {
            CliService::key_value("Since", since);
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

fn display_log_row(log: &LogEntryRow) {
    let time_part = if log.timestamp.len() >= 23 {
        &log.timestamp[11..23]
    } else {
        &log.timestamp
    };

    let line = format!(
        "{} {} [{}] {}",
        time_part, log.level, log.module, log.message
    );

    match log.level.as_str() {
        "ERROR" => CliService::error(&line),
        "WARN" => CliService::warning(&line),
        _ => CliService::info(&line),
    }
}
