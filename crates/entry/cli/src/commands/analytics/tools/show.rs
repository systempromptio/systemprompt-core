use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::{AgentUsageItem, ErrorItem, StatusBreakdownItem, ToolShowOutput, ToolStatsOutput};
use crate::commands::analytics::shared::{
    export_single_to_csv, format_duration_ms, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Tool name to analyze")]
    pub tool: String,

    #[arg(long, default_value = "24h", help = "Time range (e.g., '1h', '24h', '7d')")]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: ShowArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(&args.since, &args.until)?;
    let output = fetch_tool_details(&pool, &args.tool, start, end).await?;

    if let Some(ref path) = args.export {
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title(&format!("Tool: {}", args.tool));
        render_result(&result);
    } else {
        render_tool_details(&output);
    }

    Ok(())
}

async fn fetch_tool_details(
    pool: &std::sync::Arc<sqlx::PgPool>,
    tool_name: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<ToolShowOutput> {
    let exists: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM mcp_tool_executions WHERE tool_name ILIKE $1 AND created_at >= $2 AND created_at < $3"
    )
    .bind(format!("%{}%", tool_name))
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    if exists.0 == 0 {
        return Err(anyhow!(
            "Tool '{}' not found in the specified time range",
            tool_name
        ));
    }

    let summary = fetch_summary(pool, tool_name, start, end).await?;
    let status_breakdown = fetch_status_breakdown(pool, tool_name, start, end).await?;
    let top_errors = fetch_top_errors(pool, tool_name, start, end).await?;
    let usage_by_agent = fetch_usage_by_agent(pool, tool_name, start, end).await?;

    Ok(ToolShowOutput {
        tool_name: tool_name.to_string(),
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        summary,
        status_breakdown,
        top_errors,
        usage_by_agent,
    })
}

async fn fetch_summary(
    pool: &std::sync::Arc<sqlx::PgPool>,
    tool_name: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<ToolStatsOutput> {
    let row: (i64, i64, i64, i64, f64, f64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as total,
            COUNT(*) FILTER (WHERE status = 'success') as successful,
            COUNT(*) FILTER (WHERE status = 'failed') as failed,
            COUNT(*) FILTER (WHERE status = 'timeout') as timeout,
            COALESCE(AVG(execution_time_ms), 0) as avg_time,
            COALESCE(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY execution_time_ms), 0) as p95_time
        FROM mcp_tool_executions
        WHERE tool_name ILIKE $1
          AND created_at >= $2 AND created_at < $3
        "#,
    )
    .bind(format!("%{}%", tool_name))
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    let success_rate = if row.0 > 0 {
        (row.1 as f64 / row.0 as f64) * 100.0
    } else {
        0.0
    };

    Ok(ToolStatsOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        total_tools: 1,
        total_executions: row.0,
        successful: row.1,
        failed: row.2,
        timeout: row.3,
        success_rate,
        avg_execution_time_ms: row.4 as i64,
        p95_execution_time_ms: row.5 as i64,
    })
}

async fn fetch_status_breakdown(
    pool: &std::sync::Arc<sqlx::PgPool>,
    tool_name: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<StatusBreakdownItem>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT status, COUNT(*) as count
        FROM mcp_tool_executions
        WHERE tool_name ILIKE $1
          AND created_at >= $2 AND created_at < $3
        GROUP BY status
        ORDER BY count DESC
        "#,
    )
    .bind(format!("%{}%", tool_name))
    .bind(start)
    .bind(end)
    .fetch_all(pool.as_ref())
    .await?;

    let total: i64 = rows.iter().map(|(_, c)| c).sum();

    Ok(rows
        .into_iter()
        .map(|(status, count)| StatusBreakdownItem {
            status,
            count,
            percentage: if total > 0 {
                (count as f64 / total as f64) * 100.0
            } else {
                0.0
            },
        })
        .collect())
}

async fn fetch_top_errors(
    pool: &std::sync::Arc<sqlx::PgPool>,
    tool_name: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<ErrorItem>> {
    let rows: Vec<(Option<String>, i64)> = sqlx::query_as(
        r#"
        SELECT
            COALESCE(SUBSTRING(error_message FROM 1 FOR 100), 'Unknown error') as error_msg,
            COUNT(*) as count
        FROM mcp_tool_executions
        WHERE tool_name ILIKE $1
          AND created_at >= $2 AND created_at < $3
          AND status = 'failed'
        GROUP BY SUBSTRING(error_message FROM 1 FOR 100)
        ORDER BY count DESC
        LIMIT 10
        "#,
    )
    .bind(format!("%{}%", tool_name))
    .bind(start)
    .bind(end)
    .fetch_all(pool.as_ref())
    .await
    .unwrap_or_default();

    Ok(rows
        .into_iter()
        .map(|(error_message, count)| ErrorItem {
            error_message: error_message.unwrap_or_else(|| "Unknown".to_string()),
            count,
        })
        .collect())
}

async fn fetch_usage_by_agent(
    pool: &std::sync::Arc<sqlx::PgPool>,
    tool_name: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<AgentUsageItem>> {
    let rows: Vec<(Option<String>, i64)> = sqlx::query_as(
        r#"
        SELECT
            at.agent_name,
            COUNT(*) as count
        FROM mcp_tool_executions mte
        LEFT JOIN agent_tasks at ON at.task_id = mte.task_id
        WHERE mte.tool_name ILIKE $1
          AND mte.created_at >= $2 AND mte.created_at < $3
        GROUP BY at.agent_name
        ORDER BY count DESC
        LIMIT 10
        "#,
    )
    .bind(format!("%{}%", tool_name))
    .bind(start)
    .bind(end)
    .fetch_all(pool.as_ref())
    .await
    .unwrap_or_default();

    let total: i64 = rows.iter().map(|(_, c)| c).sum();

    Ok(rows
        .into_iter()
        .map(|(agent_name, count)| AgentUsageItem {
            agent_name: agent_name.unwrap_or_else(|| "Unknown".to_string()),
            count,
            percentage: if total > 0 {
                (count as f64 / total as f64) * 100.0
            } else {
                0.0
            },
        })
        .collect())
}

fn render_tool_details(output: &ToolShowOutput) {
    CliService::section(&format!("Tool: {} ({})", output.tool_name, output.period));

    CliService::subsection("Summary");
    CliService::key_value("Executions", &format_number(output.summary.total_executions));
    CliService::key_value("Success Rate", &format_percent(output.summary.success_rate));
    CliService::key_value(
        "Avg Duration",
        &format_duration_ms(output.summary.avg_execution_time_ms),
    );
    CliService::key_value(
        "P95 Duration",
        &format_duration_ms(output.summary.p95_execution_time_ms),
    );

    if !output.status_breakdown.is_empty() {
        CliService::subsection("Status Breakdown");
        for item in &output.status_breakdown {
            CliService::key_value(
                &item.status,
                &format!(
                    "{} ({})",
                    format_number(item.count),
                    format_percent(item.percentage)
                ),
            );
        }
    }

    if !output.top_errors.is_empty() {
        CliService::subsection("Top Errors");
        for item in &output.top_errors {
            CliService::key_value(&item.error_message, &format_number(item.count));
        }
    }

    if !output.usage_by_agent.is_empty() {
        CliService::subsection("Usage by Agent");
        for item in &output.usage_by_agent {
            CliService::key_value(
                &item.agent_name,
                &format!(
                    "{} ({})",
                    format_number(item.count),
                    format_percent(item.percentage)
                ),
            );
        }
    }
}
