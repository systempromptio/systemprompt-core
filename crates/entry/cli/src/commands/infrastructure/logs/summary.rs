use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_logging::TraceQueryService;

use super::duration::parse_since;
use crate::CliConfig;
use crate::shared::{CommandResult, RenderingHints, render_result};

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

crate::define_pool_command!(SummaryArgs => (), with_config);

async fn execute_with_pool_inner(
    args: SummaryArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let since_timestamp = parse_since(args.since.as_ref())?;
    let service = TraceQueryService::new(Arc::clone(pool));

    let (level_counts, top_modules, time_range, total_row_count) = tokio::try_join!(
        service.count_logs_by_level(since_timestamp),
        service.top_modules(since_timestamp, 10),
        service.log_time_range(since_timestamp),
        service.total_log_count(),
    )?;

    let by_level = build_level_counts(&level_counts);
    let total_logs =
        by_level.error + by_level.warn + by_level.info + by_level.debug + by_level.trace;

    let span_hours = match (&time_range.earliest, &time_range.latest) {
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
                count: r.count,
            })
            .collect(),
        time_range: TimeRange {
            earliest: time_range
                .earliest
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()),
            latest: time_range
                .latest
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()),
            span_hours,
        },
        database_info: DatabaseInfo {
            logs_table_rows: total_row_count,
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

fn build_level_counts(rows: &[systemprompt_logging::LevelCount]) -> LevelCounts {
    let mut counts = LevelCounts {
        error: 0,
        warn: 0,
        info: 0,
        debug: 0,
        trace: 0,
    };

    for row in rows {
        match row.level.to_lowercase().as_str() {
            "error" => counts.error = row.count,
            "warn" | "warning" => counts.warn = row.count,
            "info" => counts.info = row.count,
            "debug" => counts.debug = row.count,
            "trace" => counts.trace = row.count,
            _ => {},
        }
    }

    counts
}

fn render_text_output(output: &LogsSummaryOutput) {
    use systemprompt_logging::CliService;

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
