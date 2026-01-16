use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Args, ValueEnum};
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{AgentListOutput, AgentListRow};
use crate::commands::analytics::shared::{
    export_to_csv, format_cost, format_duration_ms, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum AgentSortBy {
    #[default]
    TaskCount,
    SuccessRate,
    Cost,
    LastActive,
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
        help = "Maximum number of agents"
    )]
    pub limit: i64,

    #[arg(
        long,
        value_enum,
        default_value = "task-count",
        help = "Sort by: task-count, success-rate, cost, last-active"
    )]
    pub sort_by: AgentSortBy,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: ListArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;
    execute_internal(args, &pool, config).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let pool = db_ctx.db_pool().pool_arc()?;
    execute_internal(args, &pool, config).await
}

async fn execute_internal(
    args: ListArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let output = fetch_agents(pool, start, end, args.limit, args.sort_by).await?;

    if let Some(ref path) = args.export {
        export_to_csv(&output.agents, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if output.agents.is_empty() {
        CliService::warning("No agents found in the specified time range");
        return Ok(());
    }

    if config.is_json_output() {
        let hints = RenderingHints {
            columns: Some(vec![
                "agent_name".to_string(),
                "task_count".to_string(),
                "success_rate".to_string(),
                "avg_execution_time_ms".to_string(),
                "total_cost_cents".to_string(),
            ]),
            ..Default::default()
        };
        let result = CommandResult::table(output)
            .with_title("Agent List")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_list(&output);
    }

    Ok(())
}

async fn fetch_agents(
    pool: &Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    limit: i64,
    sort_by: AgentSortBy,
) -> Result<AgentListOutput> {
    let order_clause = match sort_by {
        AgentSortBy::TaskCount => "COUNT(*) DESC",
        AgentSortBy::SuccessRate => {
            "CASE WHEN COUNT(*) > 0 THEN COUNT(*) FILTER (WHERE t.status = 'completed')::float / \
             COUNT(*)::float ELSE 0 END DESC"
        },
        AgentSortBy::Cost => "COALESCE(SUM(r.cost_cents), 0) DESC",
        AgentSortBy::LastActive => "MAX(t.started_at) DESC",
    };

    let query = format!(
        r"
        SELECT
            t.agent_name,
            COUNT(*)::bigint as task_count,
            COUNT(*) FILTER (WHERE t.status = 'completed')::bigint as completed_count,
            AVG(t.execution_time_ms::float8) as avg_execution_time_ms,
            COALESCE(SUM(r.cost_cents), 0)::bigint as total_cost_cents,
            MAX(t.started_at) as last_active
        FROM agent_tasks t
        LEFT JOIN ai_requests r ON r.task_id = t.task_id
        WHERE t.started_at >= $1 AND t.started_at < $2
          AND t.agent_name IS NOT NULL
        GROUP BY t.agent_name
        ORDER BY {}
        LIMIT $3
        ",
        order_clause
    );

    let rows: Vec<(
        Option<String>,
        i64,
        i64,
        Option<f64>,
        i64,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(&query)
        .bind(start)
        .bind(end)
        .bind(limit)
        .fetch_all(pool.as_ref())
        .await?;

    let agents: Vec<AgentListRow> = rows
        .into_iter()
        .filter_map(
            |(agent_name, task_count, completed_count, avg_time, cost, last_active)| {
                let agent_name = agent_name?;
                let success_rate = if task_count > 0 {
                    (completed_count as f64 / task_count as f64) * 100.0
                } else {
                    0.0
                };

                Some(AgentListRow {
                    agent_name,
                    task_count,
                    success_rate,
                    avg_execution_time_ms: avg_time.map_or(0, |v| v as i64),
                    total_cost_cents: cost,
                    last_active: last_active.map_or_else(
                        || "N/A".to_string(),
                        |dt| dt.format("%Y-%m-%d %H:%M:%S").to_string(),
                    ),
                })
            },
        )
        .collect();

    Ok(AgentListOutput {
        total: agents.len() as i64,
        agents,
    })
}

fn render_list(output: &AgentListOutput) {
    CliService::section("Agents");

    for agent in &output.agents {
        CliService::subsection(&agent.agent_name);
        CliService::key_value("Tasks", &format_number(agent.task_count));
        CliService::key_value("Success Rate", &format_percent(agent.success_rate));
        CliService::key_value("Avg Time", &format_duration_ms(agent.avg_execution_time_ms));
        CliService::key_value("Cost", &format_cost(agent.total_cost_cents));
        CliService::key_value("Last Active", &agent.last_active);
    }

    CliService::info(&format!("Showing {} agents", output.total));
}
