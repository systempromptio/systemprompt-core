use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

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

    let since_timestamp = parse_since(&args.since)?;

    let rows = if let Some(since_ts) = since_timestamp {
        sqlx::query_as!(
            TraceRow,
            r#"
            SELECT DISTINCT
                l.trace_id as "trace_id!",
                MIN(l.timestamp) as "first_timestamp!",
                MAX(l.timestamp) as "last_timestamp!",
                (SELECT t.agent_name FROM agent_tasks t WHERE t.trace_id = l.trace_id LIMIT 1) as "agent",
                (SELECT t.status FROM agent_tasks t WHERE t.trace_id = l.trace_id LIMIT 1) as "status",
                (SELECT COUNT(*) FROM ai_requests ar WHERE ar.trace_id = l.trace_id) as "ai_requests",
                (SELECT COUNT(*) FROM mcp_tool_executions mte WHERE mte.trace_id = l.trace_id) as "mcp_calls"
            FROM logs l
            WHERE l.trace_id IS NOT NULL
              AND l.timestamp >= $1
            GROUP BY l.trace_id
            ORDER BY MIN(l.timestamp) DESC
            LIMIT $2
            "#,
            since_ts,
            args.limit
        )
        .fetch_all(pool.as_ref())
        .await?
    } else {
        sqlx::query_as!(
            TraceRow,
            r#"
            SELECT DISTINCT
                l.trace_id as "trace_id!",
                MIN(l.timestamp) as "first_timestamp!",
                MAX(l.timestamp) as "last_timestamp!",
                (SELECT t.agent_name FROM agent_tasks t WHERE t.trace_id = l.trace_id LIMIT 1) as "agent",
                (SELECT t.status FROM agent_tasks t WHERE t.trace_id = l.trace_id LIMIT 1) as "status",
                (SELECT COUNT(*) FROM ai_requests ar WHERE ar.trace_id = l.trace_id) as "ai_requests",
                (SELECT COUNT(*) FROM mcp_tool_executions mte WHERE mte.trace_id = l.trace_id) as "mcp_calls"
            FROM logs l
            WHERE l.trace_id IS NOT NULL
            GROUP BY l.trace_id
            ORDER BY MIN(l.timestamp) DESC
            LIMIT $1
            "#,
            args.limit
        )
        .fetch_all(pool.as_ref())
        .await?
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
