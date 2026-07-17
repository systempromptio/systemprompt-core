//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::LoggingError;
pub(super) type Result<T> = std::result::Result<T, LoggingError>;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;

use systemprompt_identifiers::TraceId;

use super::models::{TraceListFilter, TraceListItem};

struct TraceRow {
    trace_id: TraceId,
    first_timestamp: DateTime<Utc>,
    last_timestamp: DateTime<Utc>,
    agent: Option<String>,
    status: String,
    ai_requests: Option<i64>,
    mcp_calls: Option<i64>,
}

pub(super) async fn list_traces(
    pool: &Arc<PgPool>,
    filter: &TraceListFilter,
) -> Result<Vec<TraceListItem>> {
    let rows = fetch_trace_rows(pool, filter).await?;

    Ok(rows
        .into_iter()
        .map(|r| TraceListItem {
            trace_id: r.trace_id,
            first_timestamp: r.first_timestamp,
            last_timestamp: r.last_timestamp,
            agent: r.agent,
            status: r.status,
            ai_requests: r.ai_requests.unwrap_or(0),
            mcp_calls: r.mcp_calls.unwrap_or(0),
        })
        .collect())
}

async fn fetch_trace_rows(pool: &Arc<PgPool>, filter: &TraceListFilter) -> Result<Vec<TraceRow>> {
    let tool_pattern = filter.tool.as_deref();
    let agent_pattern = filter.agent.as_ref().map(|a| format!("%{a}%"));
    let agent_pat = agent_pattern.as_deref();
    let status_lower = filter.status.as_ref().map(|s| s.to_lowercase());
    let status_val = status_lower.as_deref();
    let exclude_system = (!filter.include_system).then_some("1");
    let require_tracked: Option<&str> = None;

    sqlx::query_as!(
        TraceRow,
        r#"
        WITH tool_traces AS (
            SELECT DISTINCT trace_id FROM mcp_tool_executions
            WHERE ($1::text IS NULL OR tool_name ILIKE $1) AND ($2::timestamptz IS NULL OR started_at >= $2)
        ),
        all_traces AS (
            SELECT trace_id, timestamp as ts FROM logs
            WHERE trace_id IS NOT NULL AND ($2::timestamptz IS NULL OR timestamp >= $2) AND ($1::text IS NULL OR trace_id IN (SELECT trace_id FROM tool_traces))
            UNION ALL
            SELECT trace_id, created_at as ts FROM ai_requests
            WHERE trace_id IS NOT NULL AND ($2::timestamptz IS NULL OR created_at >= $2) AND ($1::text IS NULL OR trace_id IN (SELECT trace_id FROM tool_traces))
            UNION ALL
            SELECT trace_id, started_at as ts FROM mcp_tool_executions
            WHERE trace_id IS NOT NULL AND ($2::timestamptz IS NULL OR started_at >= $2) AND ($1::text IS NULL OR trace_id IN (SELECT trace_id FROM tool_traces))
            UNION ALL
            SELECT trace_id, created_at as ts FROM agent_tasks
            WHERE trace_id IS NOT NULL AND trace_id <> '' AND ($2::timestamptz IS NULL OR created_at >= $2) AND ($1::text IS NULL OR trace_id IN (SELECT trace_id FROM tool_traces))
        ),
        grouped AS (
            SELECT
                t.trace_id,
                MIN(t.ts) as first_ts,
                MAX(t.ts) as last_ts,
                (SELECT at.agent_name FROM agent_tasks at WHERE at.trace_id = t.trace_id AND at.trace_id <> '' ORDER BY at.updated_at DESC LIMIT 1) as agent,
                COALESCE(
                    CASE WHEN EXISTS (SELECT 1 FROM agent_tasks at WHERE at.trace_id = t.trace_id AND at.trace_id <> '' AND at.status IN ('TASK_STATE_PENDING','TASK_STATE_SUBMITTED','TASK_STATE_WORKING','TASK_STATE_INPUT_REQUIRED','TASK_STATE_AUTH_REQUIRED'))
                        OR EXISTS (SELECT 1 FROM ai_requests ar WHERE ar.trace_id = t.trace_id AND ar.status = 'pending')
                        OR EXISTS (SELECT 1 FROM mcp_tool_executions mte WHERE mte.trace_id = t.trace_id AND mte.status = 'pending') THEN 'running' END,
                    CASE WHEN EXISTS (SELECT 1 FROM agent_tasks at WHERE at.trace_id = t.trace_id AND at.trace_id <> '' AND at.status IN ('TASK_STATE_FAILED','TASK_STATE_REJECTED'))
                        OR EXISTS (SELECT 1 FROM ai_requests ar WHERE ar.trace_id = t.trace_id AND ar.status = 'failed')
                        OR EXISTS (SELECT 1 FROM mcp_tool_executions mte WHERE mte.trace_id = t.trace_id AND mte.status IN ('failed','timeout'))
                        OR (NOT EXISTS (SELECT 1 FROM agent_tasks at WHERE at.trace_id = t.trace_id AND at.trace_id <> '') AND NOT EXISTS (SELECT 1 FROM ai_requests ar WHERE ar.trace_id = t.trace_id) AND NOT EXISTS (SELECT 1 FROM mcp_tool_executions mte WHERE mte.trace_id = t.trace_id) AND EXISTS (SELECT 1 FROM logs l WHERE l.trace_id = t.trace_id AND l.level = 'ERROR')) THEN 'failed' END,
                    CASE WHEN EXISTS (SELECT 1 FROM agent_tasks at WHERE at.trace_id = t.trace_id AND at.trace_id <> '' AND at.status = 'TASK_STATE_CANCELED') THEN 'canceled' END,
                    'completed'
                ) as status,
                (SELECT COUNT(*) FROM ai_requests ar WHERE ar.trace_id = t.trace_id) as ai_requests,
                (SELECT COUNT(*) FROM mcp_tool_executions mte WHERE mte.trace_id = t.trace_id) as mcp_calls
            FROM all_traces t
            GROUP BY t.trace_id
        )
        SELECT trace_id as "trace_id!: TraceId", first_ts as "first_timestamp!", last_ts as "last_timestamp!",
               agent as "agent", status as "status!", ai_requests as "ai_requests", mcp_calls as "mcp_calls"
        FROM grouped
        WHERE ($3::text IS NULL OR trace_id != 'system') AND ($4::text IS NULL OR agent ILIKE $4)
          AND ($5::text IS NULL OR LOWER(status) = $5) AND ($6::boolean IS NOT TRUE OR mcp_calls > 0)
          AND ($7::text IS NULL OR status IS NOT NULL)
        ORDER BY first_ts DESC
        LIMIT $8
        "#,
        tool_pattern,
        filter.since,
        exclude_system,
        agent_pat,
        status_val,
        Some(filter.has_mcp),
        require_tracked,
        filter.limit
    )
    .fetch_all(&**pool)
    .await
    .map_err(Into::into)
}
