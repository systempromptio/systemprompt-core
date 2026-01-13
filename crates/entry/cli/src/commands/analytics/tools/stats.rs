use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::ToolStatsOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, format_duration_ms, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(
        long,
        default_value = "24h",
        help = "Time range (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, help = "Filter by tool name")]
    pub tool: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: StatsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(&args.since, &args.until)?;
    let output = fetch_stats(&pool, start, end, &args.tool).await?;

    if let Some(ref path) = args.export {
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("Tool Statistics");
        render_result(&result);
    } else {
        render_stats(&output);
    }

    Ok(())
}

async fn fetch_stats(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    tool_filter: &Option<String>,
) -> Result<ToolStatsOutput> {
    let base_query = r#"
        SELECT
            COUNT(DISTINCT tool_name) as total_tools,
            COUNT(*) as total_executions,
            COUNT(*) FILTER (WHERE status = 'success') as successful,
            COUNT(*) FILTER (WHERE status = 'failed') as failed,
            COUNT(*) FILTER (WHERE status = 'timeout') as timeout,
            COALESCE(AVG(execution_time_ms), 0) as avg_time,
            COALESCE(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY execution_time_ms), 0) as p95_time
        FROM mcp_tool_executions
        WHERE created_at >= $1 AND created_at < $2
    "#;

    let row: (i64, i64, i64, i64, i64, f64, f64) = if let Some(tool) = tool_filter {
        let query = format!("{} AND tool_name ILIKE $3", base_query);
        sqlx::query_as(&query)
            .bind(start)
            .bind(end)
            .bind(format!("%{}%", tool))
            .fetch_one(pool.as_ref())
            .await?
    } else {
        sqlx::query_as(base_query)
            .bind(start)
            .bind(end)
            .fetch_one(pool.as_ref())
            .await?
    };

    let success_rate = if row.1 > 0 {
        (row.2 as f64 / row.1 as f64) * 100.0
    } else {
        0.0
    };

    Ok(ToolStatsOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        total_tools: row.0,
        total_executions: row.1,
        successful: row.2,
        failed: row.3,
        timeout: row.4,
        success_rate,
        avg_execution_time_ms: row.5 as i64,
        p95_execution_time_ms: row.6 as i64,
    })
}

fn render_stats(output: &ToolStatsOutput) {
    CliService::section(&format!("Tool Statistics ({})", output.period));

    CliService::key_value("Total Tools", &format_number(output.total_tools));
    CliService::key_value("Total Executions", &format_number(output.total_executions));
    CliService::key_value("Successful", &format_number(output.successful));
    CliService::key_value("Failed", &format_number(output.failed));
    CliService::key_value("Timeout", &format_number(output.timeout));
    CliService::key_value("Success Rate", &format_percent(output.success_rate));
    CliService::key_value(
        "Avg Execution Time",
        &format_duration_ms(output.avg_execution_time_ms),
    );
    CliService::key_value(
        "P95 Execution Time",
        &format_duration_ms(output.p95_execution_time_ms),
    );
}
