//! `analytics agents list` command with sort options.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::{Args, ValueEnum};
use std::path::PathBuf;
use systemprompt_logging::CliService;

use super::{AgentListOutput, AgentListRow};
use crate::cli::shared::{export_to_csv, parse_time_range};
use systemprompt_extension::cli::CommandOutput;
use systemprompt_traits::usage_reports::agent::{AgentActivityReport, AgentSortOrder};

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum AgentSortBy {
    #[default]
    TaskCount,
    SuccessRate,
    Cost,
    LastActive,
}

impl From<AgentSortBy> for AgentSortOrder {
    fn from(sort: AgentSortBy) -> Self {
        match sort {
            AgentSortBy::TaskCount => Self::TaskCount,
            AgentSortBy::SuccessRate => Self::SuccessRate,
            AgentSortBy::Cost => Self::Cost,
            AgentSortBy::LastActive => Self::LastActive,
        }
    }
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(
        long,
        alias = "from",
        default_value = "24h",
        help = "Time range (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, alias = "to", help = "End time for range")]
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

pub(super) async fn execute(
    args: ListArgs,
    repo: &dyn AgentActivityReport,
) -> Result<CommandOutput> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let rows = repo
        .list_agents(start, end, args.limit, args.sort_by.into())
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
        return Ok(CommandOutput::table_of(
            vec![
                "agent_name",
                "task_count",
                "success_rate",
                "avg_execution_time_ms",
                "total_cost_microdollars",
            ],
            &output.agents,
        )
        .with_skip_render());
    }

    if output.agents.is_empty() {
        CliService::warning("No agents found in the specified time range");
        return Ok(CommandOutput::table_of(
            vec![
                "agent_name",
                "task_count",
                "success_rate",
                "avg_execution_time_ms",
                "total_cost_microdollars",
            ],
            &output.agents,
        )
        .with_skip_render());
    }

    Ok(CommandOutput::table_of(
        vec![
            "agent_name",
            "task_count",
            "success_rate",
            "avg_execution_time_ms",
            "total_cost_microdollars",
        ],
        &output.agents,
    )
    .with_title("Agent List"))
}
