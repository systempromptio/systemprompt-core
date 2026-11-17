//! `infra logs view`: list recent log entries with level, module, tail-count,
//! and since filters applied.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_logging::models::LogEntry;
use systemprompt_logging::{LogFilter, LogLevel, LoggingMaintenanceService};
use systemprompt_models::artifacts::NoticeLine;

use super::duration::parse_since;
use super::shared::filter_notes;
use super::{LogEntryRow, LogFilters, LogViewOutput};
use systemprompt_database::pool_from_handle;
use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Args)]
pub struct ViewArgs {
    #[arg(long, help = "Filter by log level (error, warn, info, debug, trace)")]
    pub level: Option<LogLevel>,

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

pub(super) async fn execute(args: ViewArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    let service = LoggingMaintenanceService::new(&pool_from_handle(host.db().await?)?);
    let logs = get_logs(&service, &args).await?;
    Ok(view_output(&build_output(&logs, &args)))
}

fn view_output(output: &LogViewOutput) -> CommandOutput {
    let table = CommandOutput::table_of(
        vec!["id", "trace_id", "timestamp", "level", "module", "message"],
        &output.logs,
    )
    .with_title("Log Entries");
    let mut notes = filter_notes(&output.filters);
    if output.logs.is_empty() {
        notes.push(NoticeLine::new("warning", "No logs found"));
    } else {
        notes.push(NoticeLine::new(
            "info",
            format!("Showing {} log entries", output.total),
        ));
        notes.push(NoticeLine::new(
            "info",
            "Tip: Use 'logs show <id>' for details or 'logs trace show <trace_id>' for full trace",
        ));
    }
    notes.into_iter().fold(table, CommandOutput::with_note)
}

fn build_filter(args: &ViewArgs) -> Result<LogFilter> {
    let since_timestamp = parse_since(args.since.as_ref())?;

    let mut filter = LogFilter::new(1, args.tail.try_into().unwrap_or(20));

    if let Some(level) = args.level {
        filter = filter.with_level(level.as_str());
    }
    if let Some(ref module) = args.module {
        filter = filter.with_module(module);
    }
    if let Some(since) = since_timestamp {
        filter = filter.with_since(since);
    }

    Ok(filter)
}

async fn get_logs(service: &LoggingMaintenanceService, args: &ViewArgs) -> Result<Vec<LogEntry>> {
    let filter = build_filter(args)?;
    let (logs, _count) = service
        .get_filtered_logs(&filter)
        .await
        .map_err(|e| anyhow!("Failed to get logs: {}", e))?;
    Ok(logs)
}

fn build_output(logs: &[LogEntry], args: &ViewArgs) -> LogViewOutput {
    let log_rows: Vec<LogEntryRow> = logs.iter().map(log_to_row).collect();

    LogViewOutput {
        total: log_rows.len() as u64,
        logs: log_rows,
        filters: LogFilters {
            level: args.level,
            module: args.module.clone(),
            since: args.since.clone(),
            pattern: None,
            tail: args.tail,
        },
    }
}

fn log_to_row(log: &LogEntry) -> LogEntryRow {
    LogEntryRow {
        id: log.id.clone(),
        trace_id: log.trace_id.clone(),
        timestamp: log.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
        level: log.level,
        module: log.module.clone(),
        message: log.message.clone(),
        metadata: log.metadata.clone(),
    }
}
