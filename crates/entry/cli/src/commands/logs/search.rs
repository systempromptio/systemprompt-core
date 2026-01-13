use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::duration::parse_since;
use super::{LogEntryRow, LogFilters, LogViewOutput};
use crate::shared::{render_result, CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct SearchArgs {
    #[arg(help = "Search pattern (matches message content)")]
    pub pattern: String,

    #[arg(long, help = "Filter by log level (error, warn, info, debug, trace)")]
    pub level: Option<String>,

    #[arg(long, help = "Filter by module name (partial match)")]
    pub module: Option<String>,

    #[arg(
        long,
        help = "Only search logs since this duration (e.g., '1h', '24h', '7d') or datetime"
    )]
    pub since: Option<String>,

    #[arg(
        long,
        short = 'n',
        default_value = "50",
        help = "Maximum number of results to return"
    )]
    pub limit: i64,
}

struct SearchRow {
    timestamp: DateTime<Utc>,
    level: String,
    module: String,
    message: String,
    metadata: Option<String>,
}

pub async fn execute(args: SearchArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let since_timestamp = parse_since(&args.since)?;
    let level_filter = args.level.as_deref().map(str::to_uppercase);

    let pattern = format!("%{}%", args.pattern);

    let rows = if let Some(since_ts) = since_timestamp {
        if let Some(ref level) = level_filter {
            sqlx::query_as!(
                SearchRow,
                r#"
                SELECT
                    timestamp as "timestamp!",
                    level as "level!",
                    module as "module!",
                    message as "message!",
                    metadata
                FROM logs
                WHERE message ILIKE $1
                  AND timestamp >= $2
                  AND UPPER(level) = $3
                ORDER BY timestamp DESC
                LIMIT $4
                "#,
                pattern,
                since_ts,
                level,
                args.limit
            )
            .fetch_all(pool.as_ref())
            .await?
        } else {
            sqlx::query_as!(
                SearchRow,
                r#"
                SELECT
                    timestamp as "timestamp!",
                    level as "level!",
                    module as "module!",
                    message as "message!",
                    metadata
                FROM logs
                WHERE message ILIKE $1
                  AND timestamp >= $2
                ORDER BY timestamp DESC
                LIMIT $3
                "#,
                pattern,
                since_ts,
                args.limit
            )
            .fetch_all(pool.as_ref())
            .await?
        }
    } else if let Some(ref level) = level_filter {
        sqlx::query_as!(
            SearchRow,
            r#"
            SELECT
                timestamp as "timestamp!",
                level as "level!",
                module as "module!",
                message as "message!",
                metadata
            FROM logs
            WHERE message ILIKE $1
              AND UPPER(level) = $2
            ORDER BY timestamp DESC
            LIMIT $3
            "#,
            pattern,
            level,
            args.limit
        )
        .fetch_all(pool.as_ref())
        .await?
    } else {
        sqlx::query_as!(
            SearchRow,
            r#"
            SELECT
                timestamp as "timestamp!",
                level as "level!",
                module as "module!",
                message as "message!",
                metadata
            FROM logs
            WHERE message ILIKE $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
            pattern,
            args.limit
        )
        .fetch_all(pool.as_ref())
        .await?
    };

    // Apply module filter if specified
    let filtered_rows: Vec<_> = if let Some(ref module) = args.module {
        rows.into_iter()
            .filter(|r| r.module.contains(module))
            .collect()
    } else {
        rows
    };

    let logs: Vec<LogEntryRow> = filtered_rows
        .into_iter()
        .map(|r| LogEntryRow {
            timestamp: r.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            level: r.level.to_uppercase(),
            module: r.module,
            message: r.message,
            metadata: r
                .metadata
                .as_ref()
                .and_then(|m| serde_json::from_str(m).ok()),
        })
        .collect();

    let output = LogViewOutput {
        total: logs.len() as u64,
        logs,
        filters: LogFilters {
            level: args.level.clone(),
            module: args.module.clone(),
            since: args.since.clone(),
            pattern: Some(args.pattern.clone()),
            tail: args.limit,
        },
    };

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
            .with_title("Search Results")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_search_results(&output, &args.pattern);
    }

    Ok(())
}

fn render_search_results(output: &LogViewOutput, pattern: &str) {
    CliService::section(&format!("Search Results: \"{}\"", pattern));

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
        CliService::warning("No matching logs found");
        return;
    }

    for log in &output.logs {
        display_log_row(log);
    }

    CliService::info(&format!("Found {} matching entries", output.total));
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
