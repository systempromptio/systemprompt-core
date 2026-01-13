use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::AgentStatsOutput;
use crate::commands::analytics::shared::{
    export_single_to_csv, format_cost, format_duration_ms, format_number, format_percent,
    parse_time_range,
};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(long, default_value = "24h", help = "Time range (e.g., '1h', '24h', '7d')")]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, help = "Filter by agent name")]
    pub agent: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: StatsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(&args.since, &args.until)?;
    let output = fetch_stats(&pool, start, end, &args.agent).await?;

    if let Some(ref path) = args.export {
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("Agent Statistics");
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
    agent_filter: &Option<String>,
) -> Result<AgentStatsOutput> {
    let (agent_condition, agent_param) = agent_filter
        .as_ref()
        .map(|a| ("AND agent_name ILIKE $3", Some(format!("%{}%", a))))
        .unwrap_or(("", None));

    let query = format!(
        r#"
        SELECT
            COUNT(DISTINCT agent_name) as total_agents,
            COUNT(*) as total_tasks,
            COUNT(*) FILTER (WHERE status = 'completed') as completed_tasks,
            COUNT(*) FILTER (WHERE status = 'failed') as failed_tasks,
            COALESCE(AVG(execution_time_ms), 0) as avg_execution_time_ms
        FROM agent_tasks
        WHERE started_at >= $1 AND started_at < $2
        {}
        "#,
        agent_condition
    );

    let row: (i64, i64, i64, i64, f64) = if let Some(ref agent) = agent_param {
        sqlx::query_as(&query)
            .bind(start)
            .bind(end)
            .bind(agent)
            .fetch_one(pool.as_ref())
            .await?
    } else {
        sqlx::query_as(&query)
            .bind(start)
            .bind(end)
            .fetch_one(pool.as_ref())
            .await?
    };

    let ai_stats: (i64, Option<i64>) = sqlx::query_as(
        r#"
        SELECT COUNT(*), SUM(cost_cents)
        FROM ai_requests
        WHERE created_at >= $1 AND created_at < $2
        "#,
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    let success_rate = if row.1 > 0 {
        (row.2 as f64 / row.1 as f64) * 100.0
    } else {
        0.0
    };

    Ok(AgentStatsOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        total_agents: row.0,
        total_tasks: row.1,
        completed_tasks: row.2,
        failed_tasks: row.3,
        success_rate,
        avg_execution_time_ms: row.4 as i64,
        total_ai_requests: ai_stats.0,
        total_cost_cents: ai_stats.1.unwrap_or(0),
    })
}

fn render_stats(output: &AgentStatsOutput) {
    CliService::section(&format!("Agent Statistics ({})", output.period));

    CliService::key_value("Active Agents", &format_number(output.total_agents));
    CliService::key_value("Total Tasks", &format_number(output.total_tasks));
    CliService::key_value("Completed", &format_number(output.completed_tasks));
    CliService::key_value("Failed", &format_number(output.failed_tasks));
    CliService::key_value("Success Rate", &format_percent(output.success_rate));
    CliService::key_value(
        "Avg Execution Time",
        &format_duration_ms(output.avg_execution_time_ms),
    );
    CliService::key_value("AI Requests", &format_number(output.total_ai_requests));
    CliService::key_value("Total Cost", &format_cost(output.total_cost_cents));
}
