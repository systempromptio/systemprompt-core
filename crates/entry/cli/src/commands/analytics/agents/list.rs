use anyhow::Result;
use clap::{Args, ValueEnum};
use std::path::PathBuf;
use systemprompt_analytics::AgentAnalyticsRepository;
use systemprompt_logging::CliService;
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

impl AgentSortBy {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::TaskCount => "task_count",
            Self::SuccessRate => "success_rate",
            Self::Cost => "cost",
            Self::LastActive => "last_active",
        }
    }
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
    let repo = AgentAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = AgentAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: ListArgs,
    repo: &AgentAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let rows = repo
        .list_agents(start, end, args.limit, args.sort_by.as_str())
        .await?;

    let agents: Vec<AgentListRow> = rows
        .into_iter()
        .map(|row| {
            let success_rate = if row.task_count > 0 {
                (row.completed_count as f64 / row.task_count as f64) * 100.0
            } else {
                0.0
            };

            AgentListRow {
                agent_name: row.agent_name,
                task_count: row.task_count,
                success_rate,
                avg_execution_time_ms: row.avg_execution_time_ms,
                total_cost_microdollars: row.total_cost_microdollars,
                last_active: row.last_active.format("%Y-%m-%d %H:%M:%S").to_string(),
            }
        })
        .collect();

    let output = AgentListOutput {
        total: agents.len() as i64,
        agents,
    };

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
                "total_cost_microdollars".to_string(),
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

fn render_list(output: &AgentListOutput) {
    CliService::section("Agents");

    for agent in &output.agents {
        CliService::subsection(&agent.agent_name);
        CliService::key_value("Tasks", &format_number(agent.task_count));
        CliService::key_value("Success Rate", &format_percent(agent.success_rate));
        CliService::key_value("Avg Time", &format_duration_ms(agent.avg_execution_time_ms));
        CliService::key_value("Cost", &format_cost(agent.total_cost_microdollars));
        CliService::key_value("Last Active", &agent.last_active);
    }

    CliService::info(&format!("Showing {} agents", output.total));
}
