//! `infra logs tools list` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use systemprompt_database::pool_from_handle;
use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_runtime::{ToolExecutionFilter, TraceQueryService, TraceRepository};

use super::{ToolExecutionRow, ToolsListOutput};
use crate::commands::infrastructure::logs::duration::parse_since;
use systemprompt_extension::cli::CommandOutput;

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

pub(in crate::commands::infrastructure::logs) async fn execute(
    args: ListArgs,
    host: &dyn KernelHost,
) -> Result<CommandOutput> {
    let pool = pool_from_handle(host.db().await?)?.pool();
    execute_with_pool_inner(args, &pool).await
}

async fn execute_with_pool_inner(
    args: ListArgs,
    pool: &Arc<sqlx::PgPool>,
) -> Result<CommandOutput> {
    let since_timestamp = parse_since(args.since.as_ref())?;
    let name_pattern = args.name.as_ref().map(|n| format!("%{n}%"));
    let server_pattern = args.server.as_ref().map(|s| format!("%{s}%"));

    let mut filter = ToolExecutionFilter::new(args.limit);
    if let Some(since) = since_timestamp {
        filter = filter.with_since(since);
    }
    if let Some(name) = name_pattern {
        filter = filter.with_name(name);
    }
    if let Some(server) = server_pattern {
        filter = filter.with_server(server);
    }
    if let Some(status) = args.status.clone() {
        filter = filter.with_status(status);
    }

    let service = TraceQueryService::new(TraceRepository::new(Arc::clone(pool)));
    let rows = service.list_tool_executions(&filter).await?;

    let executions: Vec<ToolExecutionRow> = rows
        .into_iter()
        .map(|r| ToolExecutionRow {
            timestamp: r.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
            trace_id: r.trace_id,
            tool_name: r.tool_name,
            server: r.server_name,
            status: r.status,
            duration_ms: r.execution_time_ms.map(i64::from),
        })
        .collect();

    let output = ToolsListOutput {
        total: executions.len() as u64,
        executions,
    };

    Ok(CommandOutput::table_of(
        vec![
            "timestamp",
            "trace_id",
            "tool_name",
            "server",
            "status",
            "duration_ms",
        ],
        &output.executions,
    )
    .with_title("MCP Tool Executions"))
}
