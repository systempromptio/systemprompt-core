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
    #[arg(long, default_value = "24h", help = "Time range (e.g., '1h', '24h', '7d')")]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(long, short = 'n', default_value = "20", help = "Maximum number of agents")]
    pub limit: i64,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

struct AgentRow {
    agent_name: String,
    task_count: i64,
    completed_count: i64,
    avg_execution_time_ms: Option<f64>,
    total_cost_cents: Option<i64>,
    last_active: DateTime<Utc>,
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

async fn fetch_agents(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    limit: i64,
) -> Result<AgentListOutput> {
    let rows: Vec<AgentRow> = sqlx::query_as!(
        AgentRow,
        r#"
        SELECT
            agent_name as "agent_name!",
            COUNT(*) as "task_count!",
            COUNT(*) FILTER (WHERE status = 'completed') as "completed_count!",
            AVG(execution_time_ms) as "avg_execution_time_ms",
            SUM(
                SELECT COALESCE(SUM(ar.cost_cents), 0)
                FROM ai_requests ar
                WHERE ar.task_id = agent_tasks.task_id
            ) as "total_cost_cents",
            MAX(started_at) as "last_active!"
        FROM agent_tasks
        WHERE started_at >= $1 AND started_at < $2
          AND agent_name IS NOT NULL
        GROUP BY agent_name
        ORDER BY COUNT(*) DESC
        LIMIT $3
        "#,
        start,
        end,
        limit
    )
    .fetch_all(pool.as_ref())
    .await
    .unwrap_or_else(|_| Vec::new());

    let agents: Vec<AgentListRow> = rows
        .into_iter()
        .map(|r| {
            let success_rate = if r.task_count > 0 {
                (r.completed_count as f64 / r.task_count as f64) * 100.0
            } else {
                0.0
            };

            AgentListRow {
                agent_name: r.agent_name,
                task_count: r.task_count,
                success_rate,
                avg_execution_time_ms: r.avg_execution_time_ms.map(|v| v as i64).unwrap_or(0),
                total_cost_cents: r.total_cost_cents.unwrap_or(0),
                last_active: r.last_active.format("%Y-%m-%d %H:%M:%S").to_string(),
            }
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
        CliService::key_value(
            "Avg Time",
            &format_duration_ms(agent.avg_execution_time_ms),
        );
        CliService::key_value("Cost", &format_cost(agent.total_cost_cents));
        CliService::key_value("Last Active", &agent.last_active);
    }

    CliService::info(&format!("Showing {} agents", output.total));
}
