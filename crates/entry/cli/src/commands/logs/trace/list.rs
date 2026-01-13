use anyhow::Result;
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::{TraceListOutput, TraceListRow};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, default_value = "20", help = "Maximum number of traces to return")]
    pub limit: i64,

    #[arg(long, help = "Only show traces since this duration (e.g., '1h', '24h', '7d')")]
    pub since: Option<String>,

    #[arg(long, help = "Filter by agent name")]
    pub agent: Option<String>,

    #[arg(long, help = "Filter by status (completed, failed, running)")]
    pub status: Option<String>,
}

pub async fn execute(args: ListArgs, _config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let query = r"
        SELECT DISTINCT
            l.trace_id,
            MIN(l.timestamp) as first_timestamp,
            MAX(l.timestamp) as last_timestamp,
            (SELECT t.agent_name FROM agent_tasks t WHERE t.trace_id = l.trace_id LIMIT 1) as agent,
            (SELECT t.status FROM agent_tasks t WHERE t.trace_id = l.trace_id LIMIT 1) as status,
            (SELECT COUNT(*) FROM ai_requests ar WHERE ar.trace_id = l.trace_id) as ai_requests,
            (SELECT COUNT(*) FROM mcp_tool_executions mte WHERE mte.trace_id = l.trace_id) as mcp_calls
        FROM logs l
        WHERE l.trace_id IS NOT NULL
        GROUP BY l.trace_id
        ORDER BY MIN(l.timestamp) DESC
        LIMIT $1
    ";

    let rows = sqlx::query_as::<_, (String, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>, Option<String>, Option<String>, i64, i64)>(query)
        .bind(args.limit)
        .fetch_all(pool.as_ref())
        .await?;

    let traces: Vec<TraceListRow> = rows
        .into_iter()
        .filter(|r| {
            // Apply agent filter
            if let Some(ref agent_filter) = args.agent {
                if let Some(ref agent) = r.3 {
                    if !agent.contains(agent_filter) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            // Apply status filter
            if let Some(ref status_filter) = args.status {
                if let Some(ref status) = r.4 {
                    if status.to_lowercase() != status_filter.to_lowercase() {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            true
        })
        .map(|r| {
            let duration_ms = (r.2 - r.1).num_milliseconds();
            TraceListRow {
                trace_id: r.0,
                timestamp: r.1.format("%Y-%m-%d %H:%M:%S").to_string(),
                agent: r.3,
                status: r.4.unwrap_or_else(|| "unknown".to_string()),
                duration_ms: if duration_ms > 0 { Some(duration_ms) } else { None },
                ai_requests: r.5,
                mcp_calls: r.6,
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
