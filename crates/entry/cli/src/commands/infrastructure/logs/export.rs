use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Args, ValueEnum};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::duration::parse_since;
use super::{LogEntryRow, LogExportOutput};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum ExportFormat {
    #[default]
    Json,
    Csv,
    Jsonl,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    #[arg(
        long,
        short = 'f',
        value_enum,
        default_value = "json",
        help = "Export format"
    )]
    pub format: ExportFormat,

    #[arg(long, short = 'o', help = "Output file path (stdout if not specified)")]
    pub output: Option<PathBuf>,

    #[arg(long, help = "Filter by log level")]
    pub level: Option<String>,

    #[arg(long, help = "Filter by module name")]
    pub module: Option<String>,

    #[arg(
        long,
        help = "Only export logs since this duration (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(
        long,
        default_value = "10000",
        help = "Maximum number of logs to export"
    )]
    pub limit: i64,
}

struct LogRow {
    id: String,
    trace_id: String,
    timestamp: DateTime<Utc>,
    level: String,
    module: String,
    message: String,
    metadata: Option<String>,
}

pub async fn execute(args: ExportArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool, config).await
}

pub async fn execute_with_pool(
    args: ExportArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let pool = db_ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool, config).await
}

async fn execute_with_pool_inner(
    args: ExportArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let since_timestamp = parse_since(args.since.as_ref())?;
    let level_filter = args.level.as_deref().map(str::to_uppercase);

    let rows = fetch_logs(pool, since_timestamp, level_filter.as_deref(), args.limit).await?;

    let logs: Vec<LogEntryRow> = rows
        .into_iter()
        .filter(|r| {
            args.module
                .as_ref()
                .is_none_or(|module| r.module.contains(module))
        })
        .map(|r| LogEntryRow {
            id: r.id,
            trace_id: r.trace_id,
            timestamp: r.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            level: r.level.to_uppercase(),
            module: r.module,
            message: r.message,
            metadata: r.metadata.as_ref().and_then(|m| {
                serde_json::from_str(m)
                    .map_err(|e| {
                        tracing::warn!(error = %e, "Failed to parse log metadata");
                        e
                    })
                    .ok()
            }),
        })
        .collect();

    let exported_count = logs.len() as u64;
    let format_str = match args.format {
        ExportFormat::Json => "json",
        ExportFormat::Csv => "csv",
        ExportFormat::Jsonl => "jsonl",
    };

    let content = match args.format {
        ExportFormat::Json => serde_json::to_string_pretty(&logs)?,
        ExportFormat::Jsonl => logs
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()?
            .join("\n"),
        ExportFormat::Csv => format_csv(&logs),
    };

    if let Some(ref path) = args.output {
        let mut file = std::fs::File::create(path)?;
        file.write_all(content.as_bytes())?;

        let output = LogExportOutput {
            exported_count,
            format: format_str.to_string(),
            file_path: Some(path.display().to_string()),
        };

        if config.is_json_output() {
            let result = CommandResult::card(output).with_title("Logs Exported");
            render_result(&result);
        } else {
            CliService::success(&format!(
                "Exported {} logs to {}",
                exported_count,
                path.display()
            ));
        }
    } else {
        CliService::output(&content);
    }

    Ok(())
}

async fn fetch_logs(
    pool: &Arc<sqlx::PgPool>,
    since: Option<DateTime<Utc>>,
    level: Option<&str>,
    limit: i64,
) -> Result<Vec<LogRow>> {
    let rows = if let Some(since_ts) = since {
        if let Some(level_str) = level {
            sqlx::query_as!(
                LogRow,
                r#"
                SELECT
                    id as "id!",
                    trace_id as "trace_id!",
                    timestamp as "timestamp!",
                    level as "level!",
                    module as "module!",
                    message as "message!",
                    metadata
                FROM logs
                WHERE timestamp >= $1 AND UPPER(level) = $2
                ORDER BY timestamp DESC
                LIMIT $3
                "#,
                since_ts,
                level_str,
                limit
            )
            .fetch_all(pool.as_ref())
            .await?
        } else {
            sqlx::query_as!(
                LogRow,
                r#"
                SELECT
                    id as "id!",
                    trace_id as "trace_id!",
                    timestamp as "timestamp!",
                    level as "level!",
                    module as "module!",
                    message as "message!",
                    metadata
                FROM logs
                WHERE timestamp >= $1
                ORDER BY timestamp DESC
                LIMIT $2
                "#,
                since_ts,
                limit
            )
            .fetch_all(pool.as_ref())
            .await?
        }
    } else if let Some(level_str) = level {
        sqlx::query_as!(
            LogRow,
            r#"
            SELECT
                id as "id!",
                trace_id as "trace_id!",
                timestamp as "timestamp!",
                level as "level!",
                module as "module!",
                message as "message!",
                metadata
            FROM logs
            WHERE UPPER(level) = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
            level_str,
            limit
        )
        .fetch_all(pool.as_ref())
        .await?
    } else {
        sqlx::query_as!(
            LogRow,
            r#"
            SELECT
                id as "id!",
                trace_id as "trace_id!",
                timestamp as "timestamp!",
                level as "level!",
                module as "module!",
                message as "message!",
                metadata
            FROM logs
            ORDER BY timestamp DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(pool.as_ref())
        .await?
    };

    Ok(rows)
}

fn format_csv(logs: &[LogEntryRow]) -> String {
    let mut output = String::from("id,trace_id,timestamp,level,module,message\n");

    for log in logs {
        let escaped_message = log.message.replace('"', "\"\"");
        output.push_str(&format!(
            "{},{},{},{},{},\"{}\"\n",
            log.id, log.trace_id, log.timestamp, log.level, log.module, escaped_message
        ));
    }

    output
}
