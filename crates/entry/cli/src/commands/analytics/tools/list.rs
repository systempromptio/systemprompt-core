use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Args, ValueEnum};
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::{ToolListOutput, ToolListRow};
use crate::commands::analytics::shared::{
    export_to_csv, format_duration_ms, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum ToolSortBy {
    #[default]
    ExecutionCount,
    SuccessRate,
    AvgTime,
}

struct ToolQueryParams<'a> {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    limit: i64,
    server_filter: Option<&'a String>,
    sort_by: ToolSortBy,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(
        long,
        default_value = "24h",
        help = "Time range (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(
        long,
        short = 'n',
        default_value = "20",
        help = "Maximum number of tools"
    )]
    pub limit: i64,

    #[arg(long, help = "Filter by server name")]
    pub server: Option<String>,

    #[arg(
        long,
        value_enum,
        default_value = "execution-count",
        help = "Sort by: execution-count, success-rate, avg-time"
    )]
    pub sort_by: ToolSortBy,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: ListArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let params = ToolQueryParams {
        start,
        end,
        limit: args.limit,
        server_filter: args.server.as_ref(),
        sort_by: args.sort_by,
    };
    let output = fetch_tools(&pool, &params).await?;

    if let Some(ref path) = args.export {
        export_to_csv(&output.tools, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if output.tools.is_empty() {
        CliService::warning("No tools found in the specified time range");
        return Ok(());
    }

    if config.is_json_output() {
        let hints = RenderingHints {
            columns: Some(vec![
                "tool_name".to_string(),
                "server_name".to_string(),
                "execution_count".to_string(),
                "success_rate".to_string(),
                "avg_execution_time_ms".to_string(),
            ]),
            ..Default::default()
        };
        let result = CommandResult::table(output)
            .with_title("Tool List")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_list(&output);
    }

    Ok(())
}

async fn fetch_tools(
    pool: &std::sync::Arc<sqlx::PgPool>,
    params: &ToolQueryParams<'_>,
) -> Result<ToolListOutput> {
    let order_clause = match params.sort_by {
        ToolSortBy::ExecutionCount => "COUNT(*) DESC",
        ToolSortBy::SuccessRate => "CASE WHEN COUNT(*) > 0 THEN COUNT(*) FILTER (WHERE status = 'success')::float / COUNT(*)::float ELSE 0 END DESC",
        ToolSortBy::AvgTime => "COALESCE(AVG(execution_time_ms), 0) DESC",
    };

    let base_query = r"
        SELECT
            tool_name,
            server_name,
            COUNT(*) as execution_count,
            COUNT(*) FILTER (WHERE status = 'success') as success_count,
            COALESCE(AVG(execution_time_ms)::float8, 0) as avg_time,
            MAX(created_at) as last_used
        FROM mcp_tool_executions
        WHERE created_at >= $1 AND created_at < $2
    ";

    let rows: Vec<(String, String, i64, i64, f64, DateTime<Utc>)> =
        if let Some(server) = params.server_filter {
            let query = format!(
                "{} AND server_name ILIKE $3 GROUP BY tool_name, server_name ORDER BY {} LIMIT $4",
                base_query, order_clause
            );
            sqlx::query_as(&query)
                .bind(params.start)
                .bind(params.end)
                .bind(format!("%{}%", server))
                .bind(params.limit)
                .fetch_all(pool.as_ref())
                .await?
        } else {
            let query = format!(
                "{} GROUP BY tool_name, server_name ORDER BY {} LIMIT $3",
                base_query, order_clause
            );
            sqlx::query_as(&query)
                .bind(params.start)
                .bind(params.end)
                .bind(params.limit)
                .fetch_all(pool.as_ref())
                .await?
        };

    let tools: Vec<ToolListRow> = rows
        .into_iter()
        .map(
            |(tool_name, server_name, execution_count, success_count, avg_time, last_used)| {
                let success_rate = if execution_count > 0 {
                    (success_count as f64 / execution_count as f64) * 100.0
                } else {
                    0.0
                };

                ToolListRow {
                    tool_name,
                    server_name,
                    execution_count,
                    success_rate,
                    avg_execution_time_ms: avg_time as i64,
                    last_used: last_used.format("%Y-%m-%d %H:%M:%S").to_string(),
                }
            },
        )
        .collect();

    Ok(ToolListOutput {
        total: tools.len() as i64,
        tools,
    })
}

fn render_list(output: &ToolListOutput) {
    CliService::section("Tools");

    for tool in &output.tools {
        CliService::subsection(&format!("{} ({})", tool.tool_name, tool.server_name));
        CliService::key_value("Executions", &format_number(tool.execution_count));
        CliService::key_value("Success Rate", &format_percent(tool.success_rate));
        CliService::key_value("Avg Time", &format_duration_ms(tool.avg_execution_time_ms));
        CliService::key_value("Last Used", &tool.last_used);
    }

    CliService::info(&format!("Showing {} tools", output.total));
}
