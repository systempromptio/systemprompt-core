use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::{AgentListOutput, AgentListRow};
use crate::commands::analytics::shared::{
    export_to_csv, format_cost, format_duration_ms, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult, RenderingHints};
use crate::CliConfig;

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

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: ListArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(&args.since, &args.until)?;
    let output = fetch_agents(&pool, start, end, args.limit).await?;

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

struct AgentRow {
    agent_name: Option<String>,
    task_count: Option<i64>,
    completed_count: Option<i64>,
    avg_execution_time_ms: Option<f64>,
    total_cost_cents: Option<i64>,
    last_active: Option<DateTime<Utc>>,
}

async fn fetch_agents(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    limit: i64,
) -> Result<AgentListOutput> {
    let rows = sqlx::query_as!(
        AgentRow,
        r#"
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
        ORDER BY COUNT(*) DESC
        LIMIT $3
        "#,
        start,
        end,
        limit
    )
    .fetch_all(pool.as_ref())
    .await?;

    let agents: Vec<AgentListRow> = rows
        .into_iter()
        .filter_map(|row| {
            let agent_name = row.agent_name?;
            let task_count = row.task_count.unwrap_or(0);
            let completed_count = row.completed_count.unwrap_or(0);
            let success_rate = if task_count > 0 {
                (completed_count as f64 / task_count as f64) * 100.0
            } else {
                0.0
            };

            Some(AgentListRow {
                agent_name,
                task_count,
                success_rate,
                avg_execution_time_ms: row.avg_execution_time_ms.map(|v| v as i64).unwrap_or(0),
                total_cost_cents: row.total_cost_cents.unwrap_or(0),
                last_active: row
                    .last_active
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "N/A".to_string()),
            })
        })
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
