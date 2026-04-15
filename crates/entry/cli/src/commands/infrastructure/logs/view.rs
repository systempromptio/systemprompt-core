use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_logging::models::LogEntry;
use systemprompt_logging::{CliService, LogFilter, LoggingMaintenanceService};
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::duration::parse_since;
use super::shared::display_log_row;
use super::{LogEntryRow, LogFilters, LogViewOutput};
use crate::CliConfig;
use crate::shared::{CommandResult, RenderingHints};

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

pub async fn execute(args: ViewArgs, config: &CliConfig) -> Result<CommandResult<LogViewOutput>> {
    let ctx = AppContext::new().await?;
    let service = LoggingMaintenanceService::new(ctx.db_pool())?;
    execute_inner(args, &service, config).await
}

pub async fn execute_with_pool(
    args: ViewArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<CommandResult<LogViewOutput>> {
    let service = LoggingMaintenanceService::new(db_ctx.db_pool())?;
    execute_inner(args, &service, config).await
}

async fn execute_inner(
    args: ViewArgs,
    service: &LoggingMaintenanceService,
    config: &CliConfig,
) -> Result<CommandResult<LogViewOutput>> {
    let logs = get_logs(service, &args).await?;
    let output = build_output(&logs, &args);

    let hints = RenderingHints {
        columns: Some(vec![
            "id".to_string(),
            "trace_id".to_string(),
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

    if config.is_json_output() {
        return Ok(result);
    }

    render_logs(&result.data);
    Ok(result.with_skip_render())
}

fn build_filter(args: &ViewArgs) -> Result<LogFilter> {
    let since_timestamp = parse_since(args.since.as_ref())?;

    let mut filter = LogFilter::new(1, args.tail.try_into().unwrap_or(20));

    if let Some(ref level) = args.level {
        filter = filter.with_level(level.to_uppercase());
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
        id: log.id.clone(),
        trace_id: log.trace_id.clone(),
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
    CliService::info(
        "Tip: Use 'logs show <id>' for details or 'logs trace show <trace_id>' for full trace",
    );
}
