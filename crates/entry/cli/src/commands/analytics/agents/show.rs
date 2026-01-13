use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::{
    AgentShowOutput, AgentStatsOutput, ErrorBreakdownItem, HourlyDistributionItem,
    StatusBreakdownItem,
};
use crate::commands::analytics::shared::{
    export_single_to_csv, format_duration_ms, format_number, format_percent,
    parse_time_range,
};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Agent name to analyze")]
    pub agent: String,

    #[arg(
        long,
        default_value = "24h",
        help = "Time range (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: ShowArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let output = fetch_agent_details(&pool, &args.agent, start, end).await?;

    if let Some(ref path) = args.export {
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title(format!("Agent: {}", args.agent));
        render_result(&result);
    } else {
        render_agent_details(&output);
    }

    Ok(())
}

async fn fetch_agent_details(
    pool: &std::sync::Arc<sqlx::PgPool>,
    agent_name: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<AgentShowOutput> {
    let exists: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM agent_tasks WHERE agent_name ILIKE $1 AND started_at >= $2 AND \
         started_at < $3",
    )
    .bind(format!("%{}%", agent_name))
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    if exists.0 == 0 {
        return Err(anyhow!(
            "Agent '{}' not found in the specified time range",
            agent_name
        ));
    }

    let summary = fetch_summary(pool, agent_name, start, end).await?;
    let status_breakdown = fetch_status_breakdown(pool, agent_name, start, end).await?;
    let top_errors = fetch_top_errors(pool, agent_name, start, end).await?;
    let hourly_distribution = fetch_hourly_distribution(pool, agent_name, start, end).await?;

    Ok(AgentShowOutput {
        agent_name: agent_name.to_string(),
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        summary,
        status_breakdown,
        top_errors,
        hourly_distribution,
    })
}

async fn fetch_summary(
    pool: &std::sync::Arc<sqlx::PgPool>,
    agent_name: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<AgentStatsOutput> {
    let row: (i64, i64, i64, f64) = sqlx::query_as(
        r"
        SELECT
            COUNT(*) as total_tasks,
            COUNT(*) FILTER (WHERE status = 'completed') as completed,
            COUNT(*) FILTER (WHERE status = 'failed') as failed,
            COALESCE(AVG(execution_time_ms)::float8, 0) as avg_time
        FROM agent_tasks
        WHERE agent_name ILIKE $1
          AND started_at >= $2 AND started_at < $3
        ",
    )
    .bind(format!("%{}%", agent_name))
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    let success_rate = if row.0 > 0 {
        (row.1 as f64 / row.0 as f64) * 100.0
    } else {
        0.0
    };

    Ok(AgentStatsOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        total_agents: 1,
        total_tasks: row.0,
        completed_tasks: row.1,
        failed_tasks: row.2,
        success_rate,
        avg_execution_time_ms: row.3 as i64,
        total_ai_requests: 0,
        total_cost_cents: 0,
    })
}

async fn fetch_status_breakdown(
    pool: &std::sync::Arc<sqlx::PgPool>,
    agent_name: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<StatusBreakdownItem>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        r"
        SELECT status, COUNT(*) as count
        FROM agent_tasks
        WHERE agent_name ILIKE $1
          AND started_at >= $2 AND started_at < $3
        GROUP BY status
        ORDER BY count DESC
        ",
    )
    .bind(format!("%{}%", agent_name))
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
    agent_name: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<ErrorBreakdownItem>> {
    let rows: Vec<(Option<String>, i64)> = sqlx::query_as(
        r"
        SELECT
            COALESCE(
                SUBSTRING(l.message FROM 1 FOR 100),
                'Unknown error'
            ) as error_type,
            COUNT(*) as count
        FROM agent_tasks at
        LEFT JOIN logs l ON l.task_id = at.task_id AND l.level = 'ERROR'
        WHERE at.agent_name ILIKE $1
          AND at.started_at >= $2 AND at.started_at < $3
          AND at.status = 'failed'
        GROUP BY SUBSTRING(l.message FROM 1 FOR 100)
        ORDER BY count DESC
        LIMIT 10
        ",
    )
    .bind(format!("%{}%", agent_name))
    .bind(start)
    .bind(end)
    .fetch_all(pool.as_ref())
    .await?;

    Ok(rows
        .into_iter()
        .map(|(error_type, count)| ErrorBreakdownItem {
            error_type: error_type.unwrap_or_else(|| "Unknown".to_string()),
            count,
        })
        .collect())
}

async fn fetch_hourly_distribution(
    pool: &std::sync::Arc<sqlx::PgPool>,
    agent_name: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<HourlyDistributionItem>> {
    let rows: Vec<(i32, i64)> = sqlx::query_as(
        r"
        SELECT
            EXTRACT(HOUR FROM started_at)::INTEGER as hour,
            COUNT(*) as count
        FROM agent_tasks
        WHERE agent_name ILIKE $1
          AND started_at >= $2 AND started_at < $3
        GROUP BY EXTRACT(HOUR FROM started_at)
        ORDER BY hour
        ",
    )
    .bind(format!("%{}%", agent_name))
    .bind(start)
    .bind(end)
    .fetch_all(pool.as_ref())
    .await?;

    Ok(rows
        .into_iter()
        .map(|(hour, count)| HourlyDistributionItem { hour, count })
        .collect())
}

fn render_agent_details(output: &AgentShowOutput) {
    CliService::section(&format!("Agent: {} ({})", output.agent_name, output.period));

    CliService::subsection("Summary");
    CliService::key_value("Total Tasks", &format_number(output.summary.total_tasks));
    CliService::key_value("Success Rate", &format_percent(output.summary.success_rate));
    CliService::key_value(
        "Avg Execution Time",
        &format_duration_ms(output.summary.avg_execution_time_ms),
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
            CliService::key_value(&item.error_type, &format_number(item.count));
        }
    }
}
