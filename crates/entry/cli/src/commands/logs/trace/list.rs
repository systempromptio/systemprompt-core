use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{TraceListOutput, TraceListRow};
use crate::commands::logs::duration::parse_since;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

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
}

struct TraceRow {
    trace_id: String,
    first_timestamp: DateTime<Utc>,
    last_timestamp: DateTime<Utc>,
    agent: Option<String>,
    status: Option<String>,
    ai_requests: Option<i64>,
    mcp_calls: Option<i64>,
}

pub async fn execute(args: ListArgs, _config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<()> {
    let pool = db_ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool).await
}

async fn execute_with_pool_inner(args: ListArgs, pool: &Arc<sqlx::PgPool>) -> Result<()> {
    let since_timestamp = parse_since(args.since.as_ref())?;
    let tool_pattern = args.tool.as_ref().map(|t| format!("%{}%", t));

    let rows = match (&since_timestamp, &tool_pattern) {
        (Some(since_ts), Some(tool)) => {
            sqlx::query_as!(
                TraceRow,
                r#"
                WITH tool_traces AS (
                    SELECT DISTINCT trace_id
                    FROM mcp_tool_executions
                    WHERE tool_name ILIKE $1 AND started_at >= $2
                ),
                all_traces AS (
                    SELECT t.trace_id, l.timestamp as ts
                    FROM tool_traces t
                    JOIN logs l ON l.trace_id = t.trace_id AND l.timestamp >= $2
                    UNION ALL
                    SELECT t.trace_id, ar.created_at as ts
                    FROM tool_traces t
                    JOIN ai_requests ar ON ar.trace_id = t.trace_id AND ar.created_at >= $2
                    UNION ALL
                    SELECT t.trace_id, mte.started_at as ts
                    FROM tool_traces t
                    JOIN mcp_tool_executions mte ON mte.trace_id = t.trace_id AND mte.started_at >= $2
                )
                SELECT
                    t.trace_id as "trace_id!",
                    MIN(t.ts) as "first_timestamp!",
                    MAX(t.ts) as "last_timestamp!",
                    (SELECT at.agent_name FROM agent_tasks at WHERE at.trace_id = t.trace_id LIMIT 1) as "agent",
                    (SELECT at.status FROM agent_tasks at WHERE at.trace_id = t.trace_id LIMIT 1) as "status",
                    (SELECT COUNT(*) FROM ai_requests ar WHERE ar.trace_id = t.trace_id) as "ai_requests",
                    (SELECT COUNT(*) FROM mcp_tool_executions mte WHERE mte.trace_id = t.trace_id) as "mcp_calls"
                FROM all_traces t
                GROUP BY t.trace_id
                ORDER BY MIN(t.ts) DESC
                LIMIT $3
                "#,
                tool,
                since_ts,
                args.limit
            )
            .fetch_all(pool.as_ref())
            .await?
        }
        (None, Some(tool)) => {
            sqlx::query_as!(
                TraceRow,
                r#"
                WITH tool_traces AS (
                    SELECT DISTINCT trace_id
                    FROM mcp_tool_executions
                    WHERE tool_name ILIKE $1
                ),
                all_traces AS (
                    SELECT t.trace_id, l.timestamp as ts
                    FROM tool_traces t
                    JOIN logs l ON l.trace_id = t.trace_id
                    UNION ALL
                    SELECT t.trace_id, ar.created_at as ts
                    FROM tool_traces t
                    JOIN ai_requests ar ON ar.trace_id = t.trace_id
                    UNION ALL
                    SELECT t.trace_id, mte.started_at as ts
                    FROM tool_traces t
                    JOIN mcp_tool_executions mte ON mte.trace_id = t.trace_id
                )
                SELECT
                    t.trace_id as "trace_id!",
                    MIN(t.ts) as "first_timestamp!",
                    MAX(t.ts) as "last_timestamp!",
                    (SELECT at.agent_name FROM agent_tasks at WHERE at.trace_id = t.trace_id LIMIT 1) as "agent",
                    (SELECT at.status FROM agent_tasks at WHERE at.trace_id = t.trace_id LIMIT 1) as "status",
                    (SELECT COUNT(*) FROM ai_requests ar WHERE ar.trace_id = t.trace_id) as "ai_requests",
                    (SELECT COUNT(*) FROM mcp_tool_executions mte WHERE mte.trace_id = t.trace_id) as "mcp_calls"
                FROM all_traces t
                GROUP BY t.trace_id
                ORDER BY MIN(t.ts) DESC
                LIMIT $2
                "#,
                tool,
                args.limit
            )
            .fetch_all(pool.as_ref())
            .await?
        }
        (Some(since_ts), None) => {
            sqlx::query_as!(
                TraceRow,
                r#"
                WITH all_traces AS (
                    SELECT trace_id, timestamp as ts FROM logs WHERE trace_id IS NOT NULL AND timestamp >= $1
                    UNION ALL
                    SELECT trace_id, created_at as ts FROM ai_requests WHERE trace_id IS NOT NULL AND created_at >= $1
                    UNION ALL
                    SELECT trace_id, started_at as ts FROM mcp_tool_executions WHERE trace_id IS NOT NULL AND started_at >= $1
                )
                SELECT
                    t.trace_id as "trace_id!",
                    MIN(t.ts) as "first_timestamp!",
                    MAX(t.ts) as "last_timestamp!",
                    (SELECT at.agent_name FROM agent_tasks at WHERE at.trace_id = t.trace_id LIMIT 1) as "agent",
                    (SELECT at.status FROM agent_tasks at WHERE at.trace_id = t.trace_id LIMIT 1) as "status",
                    (SELECT COUNT(*) FROM ai_requests ar WHERE ar.trace_id = t.trace_id) as "ai_requests",
                    (SELECT COUNT(*) FROM mcp_tool_executions mte WHERE mte.trace_id = t.trace_id) as "mcp_calls"
                FROM all_traces t
                GROUP BY t.trace_id
                ORDER BY MIN(t.ts) DESC
                LIMIT $2
                "#,
                since_ts,
                args.limit
            )
            .fetch_all(pool.as_ref())
            .await?
        }
        (None, None) => {
            sqlx::query_as!(
                TraceRow,
                r#"
                WITH all_traces AS (
                    SELECT trace_id, timestamp as ts FROM logs WHERE trace_id IS NOT NULL
                    UNION ALL
                    SELECT trace_id, created_at as ts FROM ai_requests WHERE trace_id IS NOT NULL
                    UNION ALL
                    SELECT trace_id, started_at as ts FROM mcp_tool_executions WHERE trace_id IS NOT NULL
                )
                SELECT
                    t.trace_id as "trace_id!",
                    MIN(t.ts) as "first_timestamp!",
                    MAX(t.ts) as "last_timestamp!",
                    (SELECT at.agent_name FROM agent_tasks at WHERE at.trace_id = t.trace_id LIMIT 1) as "agent",
                    (SELECT at.status FROM agent_tasks at WHERE at.trace_id = t.trace_id LIMIT 1) as "status",
                    (SELECT COUNT(*) FROM ai_requests ar WHERE ar.trace_id = t.trace_id) as "ai_requests",
                    (SELECT COUNT(*) FROM mcp_tool_executions mte WHERE mte.trace_id = t.trace_id) as "mcp_calls"
                FROM all_traces t
                GROUP BY t.trace_id
                ORDER BY MIN(t.ts) DESC
                LIMIT $1
                "#,
                args.limit
            )
            .fetch_all(pool.as_ref())
            .await?
        }
    };

    let traces: Vec<TraceListRow> = rows
        .into_iter()
        .filter(|r| matches_filters(r, &args))
        .map(row_to_trace_list)
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

fn matches_filters(row: &TraceRow, args: &ListArgs) -> bool {
    if let Some(ref agent_filter) = args.agent {
        match &row.agent {
            Some(agent) if agent.contains(agent_filter) => {},
            _ => return false,
        }
    }

    if let Some(ref status_filter) = args.status {
        match &row.status {
            Some(status) if status.eq_ignore_ascii_case(status_filter) => {},
            _ => return false,
        }
    }

    if args.has_mcp && row.mcp_calls.unwrap_or(0) == 0 {
        return false;
    }

    true
}

fn row_to_trace_list(r: TraceRow) -> TraceListRow {
    let duration_ms = (r.last_timestamp - r.first_timestamp).num_milliseconds();

    TraceListRow {
        trace_id: r.trace_id,
        timestamp: r.first_timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
        agent: r.agent,
        status: r.status.unwrap_or_else(|| "unknown".to_string()),
        duration_ms: (duration_ms > 0).then_some(duration_ms),
        ai_requests: r.ai_requests.unwrap_or(0),
        mcp_calls: r.mcp_calls.unwrap_or(0),
    }
}
