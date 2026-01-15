use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_runtime::AppContext;

use super::duration::parse_since;
use crate::shared::{render_result, CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct SummaryArgs {
    #[arg(
        long,
        help = "Only include logs since this duration (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogsSummaryOutput {
    pub total_logs: i64,
    pub by_level: LevelCounts,
    pub top_modules: Vec<ModuleCount>,
    pub time_range: TimeRange,
    pub database_info: DatabaseInfo,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct LevelCounts {
    pub error: i64,
    pub warn: i64,
    pub info: i64,
    pub debug: i64,
    pub trace: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModuleCount {
    pub module: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TimeRange {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub earliest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_hours: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DatabaseInfo {
    pub logs_table_rows: i64,
}

struct LevelRow {
    level: String,
    count: Option<i64>,
}

struct ModuleRow {
    module: String,
    count: Option<i64>,
}

struct TimeRangeRow {
    earliest: Option<DateTime<Utc>>,
    latest: Option<DateTime<Utc>>,
}

pub async fn execute(args: SummaryArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let since_timestamp = parse_since(args.since.as_ref())?;

    // Get counts by level
    let level_counts = if let Some(since_ts) = since_timestamp {
        sqlx::query_as!(
            LevelRow,
            r#"
            SELECT level as "level!", COUNT(*) as "count"
            FROM logs
            WHERE timestamp >= $1
            GROUP BY level
            "#,
            since_ts
        )
        .fetch_all(pool.as_ref())
        .await?
    } else {
        sqlx::query_as!(
            LevelRow,
            r#"
            SELECT level as "level!", COUNT(*) as "count"
            FROM logs
            GROUP BY level
            "#
        )
        .fetch_all(pool.as_ref())
        .await?
    };

    let by_level = build_level_counts(&level_counts);
    let total_logs =
        by_level.error + by_level.warn + by_level.info + by_level.debug + by_level.trace;

    // Get top modules
    let top_modules = if let Some(since_ts) = since_timestamp {
        sqlx::query_as!(
            ModuleRow,
            r#"
            SELECT module as "module!", COUNT(*) as "count"
            FROM logs
            WHERE timestamp >= $1
            GROUP BY module
            ORDER BY count DESC
            LIMIT 10
            "#,
            since_ts
        )
        .fetch_all(pool.as_ref())
        .await?
    } else {
        sqlx::query_as!(
            ModuleRow,
            r#"
            SELECT module as "module!", COUNT(*) as "count"
            FROM logs
            GROUP BY module
            ORDER BY count DESC
            LIMIT 10
            "#
        )
        .fetch_all(pool.as_ref())
        .await?
    };

    // Get time range
    let time_range_row = if let Some(since_ts) = since_timestamp {
        sqlx::query_as!(
            TimeRangeRow,
            r#"
            SELECT MIN(timestamp) as "earliest", MAX(timestamp) as "latest"
            FROM logs
            WHERE timestamp >= $1
            "#,
            since_ts
        )
        .fetch_one(pool.as_ref())
        .await?
    } else {
        sqlx::query_as!(
            TimeRangeRow,
            r#"
            SELECT MIN(timestamp) as "earliest", MAX(timestamp) as "latest"
            FROM logs
            "#
        )
        .fetch_one(pool.as_ref())
        .await?
    };

    // Get total row count (for database info)
    let total_row_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM logs")
        .fetch_one(pool.as_ref())
        .await?;

    let span_hours = match (&time_range_row.earliest, &time_range_row.latest) {
        (Some(e), Some(l)) => Some((*l - *e).num_hours()),
        _ => None,
    };

    let output = LogsSummaryOutput {
        total_logs,
        by_level,
        top_modules: top_modules
            .into_iter()
            .map(|r| ModuleCount {
                module: r.module,
                count: r.count.unwrap_or(0),
            })
            .collect(),
        time_range: TimeRange {
            earliest: time_range_row
                .earliest
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()),
            latest: time_range_row
                .latest
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()),
            span_hours,
        },
        database_info: DatabaseInfo {
            logs_table_rows: total_row_count.0,
        },
    };

    if config.is_json_output() {
        let hints = RenderingHints::default();
        let result = CommandResult::card(output)
            .with_title("Logs Summary")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_text_output(&output);
    }

    Ok(())
}

fn build_level_counts(rows: &[LevelRow]) -> LevelCounts {
    let mut counts = LevelCounts {
        error: 0,
        warn: 0,
        info: 0,
        debug: 0,
        trace: 0,
    };

    for row in rows {
        let count = row.count.unwrap_or(0);
        match row.level.to_lowercase().as_str() {
            "error" => counts.error = count,
            "warn" | "warning" => counts.warn = count,
            "info" => counts.info = count,
            "debug" => counts.debug = count,
            "trace" => counts.trace = count,
            _ => {},
        }
    }

    counts
}

fn render_text_output(output: &LogsSummaryOutput) {
    use systemprompt_core_logging::CliService;

    CliService::section("Logs Summary");

    CliService::key_value("Total Logs", &output.total_logs.to_string());

    CliService::subsection("By Level");
    if output.by_level.error > 0 {
        CliService::error(&format!("  Errors:   {}", output.by_level.error));
    } else {
        CliService::key_value("  Errors", &output.by_level.error.to_string());
    }
    if output.by_level.warn > 0 {
        CliService::warning(&format!("  Warnings: {}", output.by_level.warn));
    } else {
        CliService::key_value("  Warnings", &output.by_level.warn.to_string());
    }
    CliService::key_value("  Info", &output.by_level.info.to_string());
    CliService::key_value("  Debug", &output.by_level.debug.to_string());
    CliService::key_value("  Trace", &output.by_level.trace.to_string());

    if !output.top_modules.is_empty() {
        CliService::subsection("Top Modules");
        for module in &output.top_modules {
            CliService::info(&format!("  {} ({})", module.module, module.count));
        }
    }

    CliService::subsection("Time Range");
    if let Some(ref earliest) = output.time_range.earliest {
        CliService::key_value("  Earliest", earliest);
    }
    if let Some(ref latest) = output.time_range.latest {
        CliService::key_value("  Latest", latest);
    }
    if let Some(span) = output.time_range.span_hours {
        CliService::key_value("  Span", &format!("{} hours", span));
    }

    CliService::subsection("Database");
    CliService::key_value(
        "  Total Rows",
        &output.database_info.logs_table_rows.to_string(),
    );
}
