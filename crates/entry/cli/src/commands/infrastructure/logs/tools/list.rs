use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::queries::query_tools;
use super::{ToolExecutionRow, ToolsListOutput};
use crate::commands::infrastructure::logs::duration::parse_since;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, help = "Filter by tool name (partial match)")]
    pub name: Option<String>,

    #[arg(long, help = "Filter by server name (partial match)")]
    pub server: Option<String>,

    #[arg(long, help = "Filter by status (success, error, pending)")]
    pub status: Option<String>,

    #[arg(long, help = "Only show executions since duration (e.g., '1h', '7d')")]
    pub since: Option<String>,

    #[arg(long, short = 'n', default_value = "50", help = "Maximum results")]
    pub limit: i64,
}

pub async fn execute(args: ListArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool, config).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let pool = db_ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool, config).await
}

async fn execute_with_pool_inner(
    args: ListArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let since_timestamp = parse_since(args.since.as_ref())?;
    let name_pattern = args.name.as_ref().map(|n| format!("%{}%", n));
    let server_pattern = args.server.as_ref().map(|s| format!("%{}%", s));

    let rows = query_tools(
        pool,
        since_timestamp,
        name_pattern.as_deref(),
        server_pattern.as_deref(),
        args.status.as_deref(),
        args.limit,
    )
    .await?;

    let executions: Vec<ToolExecutionRow> = rows
        .into_iter()
        .map(|r| ToolExecutionRow {
            timestamp: r.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
            trace_id: r.trace_id,
            tool_name: r.tool_name,
            server: r.server_name.unwrap_or_else(|| "unknown".to_string()),
            status: r.status,
            duration_ms: r.execution_time_ms.map(i64::from),
        })
        .collect();

    let output = ToolsListOutput {
        total: executions.len() as u64,
        executions,
    };

    if config.is_json_output() {
        let result = CommandResult::table(output)
            .with_title("MCP Tool Executions")
            .with_columns(vec![
                "timestamp".to_string(),
                "trace_id".to_string(),
                "tool_name".to_string(),
                "server".to_string(),
                "status".to_string(),
                "duration_ms".to_string(),
            ]);
        render_result(&result);
    } else {
        render_tool_list(&output, &args);
    }

    Ok(())
}

fn render_tool_list(output: &ToolsListOutput, args: &ListArgs) {
    CliService::section("MCP Tool Executions");

    if args.name.is_some() || args.server.is_some() || args.status.is_some() || args.since.is_some()
    {
        if let Some(ref name) = args.name {
            CliService::key_value("Tool", name);
        }
        if let Some(ref server) = args.server {
            CliService::key_value("Server", server);
        }
        if let Some(ref status) = args.status {
            CliService::key_value("Status", status);
        }
        if let Some(ref since) = args.since {
            CliService::key_value("Since", since);
        }
    }

    if output.executions.is_empty() {
        CliService::warning("No tool executions found");
        return;
    }

    for exec in &output.executions {
        let duration = exec.duration_ms.map(|d| format!(" ({}ms)", d));
        let line = format!(
            "{} {}/{} [{}]{}  trace:{}",
            exec.timestamp,
            exec.server,
            exec.tool_name,
            exec.status,
            duration.as_deref().unwrap_or(""),
            exec.trace_id
        );
        match exec.status.as_str() {
            "error" | "failed" => CliService::error(&line),
            _ => CliService::info(&line),
        }
    }

    CliService::info(&format!("Total: {} executions", output.total));
}
