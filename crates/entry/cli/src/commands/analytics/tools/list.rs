use anyhow::Result;
use clap::{Args, ValueEnum};
use std::path::PathBuf;
use systemprompt_analytics::ToolAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{ToolListOutput, ToolListRow};
use crate::commands::analytics::shared::{
    export_to_csv, format_duration_ms, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult, RenderingHints};
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

pub async fn execute(args: ListArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let repo = ToolAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = ToolAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: ListArgs,
    repo: &ToolAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
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
        export_to_csv(&output.tools, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if output.tools.is_empty() {
        CliService::warning("No tools found in the specified time range");
        return Ok(());
    }

    if config.is_json_output() {
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
        let result = CommandResult::table(output)
            .with_title("Tool List")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_list(&output);
    }

    Ok(())
}

fn render_list(output: &ToolListOutput) {
    CliService::section("Tools");

    for tool in &output.tools {
        CliService::subsection(&format!("{} ({})", tool.tool_name, tool.server_name));
        CliService::key_value("Executions", &format_number(tool.execution_count));
        CliService::key_value("Success Rate", &format_percent(tool.success_rate));
        CliService::key_value("Avg Time", &format_duration_ms(tool.avg_execution_time_ms));
        CliService::key_value("Last Used", &tool.last_used);
    }

    CliService::info(&format!("Showing {} tools", output.total));
}
