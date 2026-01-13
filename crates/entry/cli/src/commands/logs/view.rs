use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use clap::Args;
use std::sync::Arc;
use systemprompt_core_logging::models::{LogEntry, LogLevel};
use systemprompt_core_logging::{CliService, LoggingMaintenanceService};
use systemprompt_runtime::AppContext;

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

fn parse_since(since: &Option<String>) -> Result<Option<DateTime<Utc>>> {
    let Some(s) = since else {
        return Ok(None);
    };

    let s = s.trim().to_lowercase();

    // Try duration format first (e.g., "1h", "24h", "7d")
    if let Some(duration) = parse_duration(&s) {
        return Ok(Some(Utc::now() - duration));
    }

    // Try parsing as date (e.g., "2026-01-13")
    if let Ok(date) = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        let datetime = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Some(DateTime::from_naive_utc_and_offset(datetime, Utc)));
    }

    // Try parsing as datetime (e.g., "2026-01-13T10:00:00")
    if let Ok(datetime) = chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(Some(DateTime::from_naive_utc_and_offset(datetime, Utc)));
    }

    Err(anyhow!(
        "Invalid --since format: {}. Use formats like '1h', '24h', '7d', '2026-01-13', or '2026-01-13T10:00:00'",
        s
    ))
}

fn parse_duration(s: &str) -> Option<Duration> {
    if let Some(days) = s.strip_suffix('d') {
        let num: i64 = days.parse().ok()?;
        return Some(Duration::days(num));
    }

    if let Some(hours) = s.strip_suffix('h') {
        let num: i64 = hours.parse().ok()?;
        return Some(Duration::hours(num));
    }

    if let Some(mins) = s.strip_suffix('m') {
        let num: i64 = mins.parse().ok()?;
        return Some(Duration::minutes(num));
    }

    None
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
