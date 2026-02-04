use anyhow::Result;
use clap::{Args, ValueEnum};
use std::path::PathBuf;
use systemprompt_analytics::ToolAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{ToolListOutput, ToolListRow};
use crate::commands::analytics::shared::{export_to_csv, parse_time_range, resolve_export_path};
use crate::shared::{CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum ToolSortBy {
    #[default]
    ExecutionCount,
    SuccessRate,
    AvgTime,
}

impl ToolSortBy {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::ExecutionCount => "execution_count",
            Self::SuccessRate => "success_rate",
            Self::AvgTime => "avg_time",
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

pub async fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandResult<ToolListOutput>> {
    let ctx = AppContext::new().await?;
    let repo = ToolAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<ToolListOutput>> {
    let repo = ToolAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: ListArgs,
    repo: &ToolAnalyticsRepository,
) -> Result<CommandResult<ToolListOutput>> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let rows = repo
        .list_tools(
            start,
            end,
            args.limit,
            args.server.as_deref(),
            args.sort_by.as_str(),
        )
        .await?;

    let tools: Vec<ToolListRow> = rows
        .into_iter()
        .map(|row| {
            let success_rate = if row.execution_count > 0 {
                (row.success_count as f64 / row.execution_count as f64) * 100.0
            } else {
                0.0
            };

            ToolListRow {
                tool_name: row.tool_name,
                server_name: row.server_name,
                execution_count: row.execution_count,
                success_rate,
                avg_execution_time_ms: row.avg_time as i64,
                last_used: row.last_used.format("%Y-%m-%d %H:%M:%S").to_string(),
            }
        })
        .collect();

    let output = ToolListOutput {
        total: tools.len() as i64,
        tools,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_to_csv(&output.tools, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::table(output).with_skip_render());
    }

    if output.tools.is_empty() {
        CliService::warning("No tools found in the specified time range");
        return Ok(CommandResult::table(output).with_skip_render());
    }

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

    Ok(CommandResult::table(output)
        .with_title("Tool List")
        .with_hints(hints))
}
