use anyhow::Result;
use clap::{Args, ValueEnum};
use std::path::PathBuf;
use systemprompt_analytics::AgentAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{AgentListOutput, AgentListRow};
use crate::commands::analytics::shared::{export_to_csv, parse_time_range, resolve_export_path};
use crate::shared::{CommandResult, RenderingHints};
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

pub async fn execute(
    args: ListArgs,
    _config: &CliConfig,
) -> Result<CommandResult<AgentListOutput>> {
    let ctx = AppContext::new().await?;
    let repo = AgentAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<AgentListOutput>> {
    let repo = AgentAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: ListArgs,
    repo: &AgentAnalyticsRepository,
) -> Result<CommandResult<AgentListOutput>> {
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
        let resolved_path = resolve_export_path(path)?;
        export_to_csv(&output.agents, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::table(output).with_skip_render());
    }

    if output.agents.is_empty() {
        CliService::warning("No agents found in the specified time range");
        return Ok(CommandResult::table(output).with_skip_render());
    }

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

    Ok(CommandResult::table(output)
        .with_title("Agent List")
        .with_hints(hints))
}
