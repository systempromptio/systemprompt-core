use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_logging::{CliService, TraceListFilter, TraceQueryService};

use super::{TraceListOutput, TraceListRow};
use crate::commands::infrastructure::logs::duration::parse_since;
use crate::shared::{CommandResult, render_result};

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(
        long,
        short = 'n',
        default_value = "20",
        help = "Maximum number of traces to return"
    )]
    pub limit: i64,

    #[arg(
        long,
        help = "Only show traces since this duration (e.g., '1h', '24h', '7d') or datetime"
    )]
    pub since: Option<String>,

    #[arg(long, help = "Filter by agent name")]
    pub agent: Option<String>,

    #[arg(long, help = "Filter by status (completed, failed, running)")]
    pub status: Option<String>,

    #[arg(
        long,
        help = "Filter by MCP tool name (shows only traces that used this tool)"
    )]
    pub tool: Option<String>,

    #[arg(long, help = "Only show traces with MCP tool calls")]
    pub has_mcp: bool,

    #[arg(long, help = "Include system and untracked traces")]
    pub all: bool,
}

crate::define_pool_command!(ListArgs => (), no_config);

async fn execute_with_pool_inner(args: ListArgs, pool: &Arc<sqlx::PgPool>) -> Result<()> {
    let since_timestamp = parse_since(args.since.as_ref())?;
    let tool_pattern = args.tool.as_ref().map(|t| format!("%{}%", t));

    let mut filter = TraceListFilter::new(args.limit)
        .with_has_mcp(args.has_mcp)
        .with_include_system(args.all);

    if let Some(since) = since_timestamp {
        filter = filter.with_since(since);
    }
    if let Some(agent) = args.agent.clone() {
        filter = filter.with_agent(agent);
    }
    if let Some(status) = args.status.clone() {
        filter = filter.with_status(status);
    }
    if let Some(tool) = tool_pattern {
        filter = filter.with_tool(tool);
    }

    let service = TraceQueryService::new(Arc::clone(pool));
    let items = service.list_traces(&filter).await?;

    let traces: Vec<TraceListRow> = items
        .into_iter()
        .map(|r| {
            let duration_ms = (r.last_timestamp - r.first_timestamp).num_milliseconds();
            TraceListRow {
                trace_id: r.trace_id,
                timestamp: r.first_timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
                agent: r.agent,
                status: r.status.unwrap_or_else(|| "unknown".to_string()),
                duration_ms: (duration_ms > 0).then_some(duration_ms),
                ai_requests: r.ai_requests,
                mcp_calls: r.mcp_calls,
            }
        })
        .collect();

    let output = TraceListOutput {
        total: traces.len() as u64,
        traces,
    };

    if output.traces.is_empty() {
        CliService::warning("No traces found");
        return Ok(());
    }

    let result = CommandResult::table(output)
        .with_title("Recent Traces")
        .with_columns(vec![
            "trace_id".to_string(),
            "timestamp".to_string(),
            "agent".to_string(),
            "status".to_string(),
            "duration_ms".to_string(),
            "ai_requests".to_string(),
            "mcp_calls".to_string(),
        ]);

    render_result(&result);

    Ok(())
}
