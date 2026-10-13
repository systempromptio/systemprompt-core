//! Trace listing over the stored `trace_facts` projection, one row per trace,
//! with filter-driven pagination.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};

use systemprompt_identifiers::TraceId;

use super::{Result, TraceRepository};
use crate::trace::models::{TraceListFilter, TraceListItem};

struct TraceRow {
    trace_id: TraceId,
    first_timestamp: DateTime<Utc>,
    last_timestamp: DateTime<Utc>,
    agent: Option<String>,
    status: String,
    ai_requests: i64,
    mcp_calls: i64,
}

impl TraceRepository {
    pub async fn list_traces(&self, filter: &TraceListFilter) -> Result<Vec<TraceListItem>> {
        let rows = self.fetch_trace_rows(filter).await?;

        Ok(rows
            .into_iter()
            .map(|r| TraceListItem {
                trace_id: r.trace_id,
                first_timestamp: r.first_timestamp,
                last_timestamp: r.last_timestamp,
                agent: r.agent,
                status: r.status,
                ai_requests: r.ai_requests,
                mcp_calls: r.mcp_calls,
            })
            .collect())
    }

    async fn fetch_trace_rows(&self, filter: &TraceListFilter) -> Result<Vec<TraceRow>> {
        let tool_pattern = filter.tool.as_deref();
        let agent_pattern = filter.agent.as_ref().map(|a| format!("%{a}%"));
        let agent_pat = agent_pattern.as_deref();
        let status_lower = filter.status.as_ref().map(|s| s.to_lowercase());
        let status_val = status_lower.as_deref();
        let exclude_system = !filter.include_system;

        sqlx::query_as!(
            TraceRow,
            r#"
            SELECT trace_id AS "trace_id!: TraceId", first_at AS "first_timestamp!",
                   last_at AS "last_timestamp!", agent_name AS "agent", status AS "status!",
                   ai_count AS "ai_requests!", mcp_count AS "mcp_calls!"
            FROM trace_facts
            WHERE ($2::timestamptz IS NULL OR last_at >= $2)
              AND ($1::text IS NULL OR EXISTS (SELECT 1 FROM unnest(tool_names) AS t(name) WHERE t.name ILIKE $1))
              AND (NOT $3::boolean OR trace_id <> 'system')
              AND (NOT $3::boolean OR ai_count > 0 OR mcp_count > 0 OR agent_name IS NOT NULL)
              AND ($4::text IS NULL OR agent_name ILIKE $4)
              AND ($5::text IS NULL OR LOWER(status) = $5)
              AND ($6::boolean IS NOT TRUE OR mcp_count > 0)
            ORDER BY first_at DESC
            LIMIT $7
            "#,
            tool_pattern,
            filter.since,
            exclude_system,
            agent_pat,
            status_val,
            Some(filter.has_mcp),
            filter.limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }
}
