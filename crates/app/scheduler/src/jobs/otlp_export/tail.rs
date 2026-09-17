//! Reads for the exporter: the next batch of a signal after its watermark,
//! and the child rows that hang off a batch of requests.
//!
//! Every tail leaves a settle window untouched at the head: a row is only
//! taken once it is older than [`SETTLE`], so a transaction that commits
//! late with an earlier timestamp cannot land behind an advanced cursor.
//! [`BATCH_ROWS`] bounds one batch per signal per tick.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_identifiers::{ClientId, ContextId, SessionId, TraceId, UserId};

use super::state::Watermark;
use crate::error::SchedulerResult;

pub const SETTLE: Duration = Duration::from_secs(5);

pub const BATCH_ROWS: i64 = 500;

#[derive(Debug, Clone)]
pub struct RequestRow {
    pub id: String,
    pub request_id: String,
    pub user_id: UserId,
    pub session_id: Option<SessionId>,
    pub context_id: ContextId,
    pub trace_id: Option<TraceId>,
    pub provider: Option<String>,
    pub served_provider: Option<String>,
    pub model: Option<String>,
    pub requested_model: Option<String>,
    pub route_match: Option<String>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cache_read_tokens: Option<i32>,
    pub cache_creation_tokens: Option<i32>,
    pub cost_microdollars: i64,
    pub latency_ms: Option<i32>,
    pub upstream_latency_ms: Option<i32>,
    pub finish_reason: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub client_kind: String,
    pub wire_protocol: String,
    pub request_kind: String,
    pub actor_kind: String,
    pub actor_id: String,
    pub instance_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LedgerRow {
    pub ai_tool_call_id: Option<String>,
    pub request_id: Option<String>,
    pub mcp_execution_id: Option<String>,
    pub tool_name: Option<String>,
    pub server_name: Option<String>,
    pub intended_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub execution_time_ms: Option<i32>,
    pub execution_status: Option<String>,
    pub error_message: Option<String>,
    pub source: Option<String>,
    pub state: Option<String>,
    pub is_error: Option<bool>,
    pub artifact_type: Option<String>,
    pub payload_bytes: Option<i32>,
    pub secret_redactions: Option<i32>,
    pub occurred_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct GovernanceRow {
    pub id: String,
    pub trace_id: Option<TraceId>,
    pub tool_name: String,
    pub decision: String,
    pub policy: String,
    pub reason: String,
    pub plugin_id: Option<String>,
    pub actor_kind: String,
    pub actor_id: String,
    pub tool_use_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LogRow {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub module: String,
    pub message: String,
    pub metadata: Option<String>,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub context_id: Option<ContextId>,
    pub client_id: Option<ClientId>,
    pub instance_id: Option<String>,
    pub provider_request_id: Option<String>,
    pub gateway_conversation_id: Option<String>,
}

pub(super) async fn list_requests_after(
    pool: &PgPool,
    after: &Watermark,
    limit: i64,
) -> SchedulerResult<Vec<RequestRow>> {
    let rows = sqlx::query_as!(
        RequestRow,
        r#"
        SELECT id, request_id, user_id AS "user_id: UserId", session_id AS "session_id: SessionId",
               context_id AS "context_id: ContextId", trace_id AS "trace_id: TraceId", provider,
               served_provider, model, requested_model, route_match, input_tokens,
               output_tokens, cache_read_tokens, cache_creation_tokens, cost_microdollars,
               latency_ms, upstream_latency_ms, finish_reason, status, error_message,
               client_kind, wire_protocol, request_kind, actor_kind, actor_id, instance_id,
               created_at, completed_at AS "completed_at!"
        FROM ai_requests
        WHERE completed_at IS NOT NULL
          AND (completed_at > $1 OR (completed_at = $1 AND id > $2))
          AND completed_at <= NOW() - make_interval(secs => $3::DOUBLE PRECISION)
        ORDER BY completed_at, id
        LIMIT $4
        "#,
        after.at,
        after.id,
        SETTLE.as_secs_f64(),
        limit
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub(super) async fn list_ledger_for_requests(
    pool: &PgPool,
    request_ids: &[String],
) -> SchedulerResult<Vec<LedgerRow>> {
    let rows = sqlx::query_as!(
        LedgerRow,
        r#"
        SELECT ai_tool_call_id, request_id, mcp_execution_id, tool_name, server_name,
               intended_at, executed_at, completed_at, execution_time_ms, execution_status,
               error_message, source, state, is_error, artifact_type, payload_bytes,
               secret_redactions, occurred_at
        FROM tool_call_ledger
        WHERE request_id = ANY($1)
        ORDER BY occurred_at
        "#,
        request_ids
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub(super) async fn list_governance_for_traces(
    pool: &PgPool,
    trace_ids: &[String],
) -> SchedulerResult<Vec<GovernanceRow>> {
    let rows = sqlx::query_as!(
        GovernanceRow,
        r#"
        SELECT id, trace_id AS "trace_id: TraceId", tool_name, decision, policy, reason, plugin_id, actor_kind,
               actor_id, tool_use_id, created_at
        FROM governance_decisions
        WHERE trace_id = ANY($1)
        ORDER BY created_at
        "#,
        trace_ids
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub(super) async fn list_logs_after(
    pool: &PgPool,
    after: &Watermark,
    limit: i64,
) -> SchedulerResult<Vec<LogRow>> {
    let rows = sqlx::query_as!(
        LogRow,
        r#"
        SELECT id, timestamp, level, module, message, metadata, user_id AS "user_id: UserId",
               session_id AS "session_id: SessionId", trace_id AS "trace_id: TraceId",
               context_id AS "context_id: ContextId", client_id AS "client_id: ClientId",
               instance_id, provider_request_id, gateway_conversation_id
        FROM logs
        WHERE (timestamp > $1 OR (timestamp = $1 AND id > $2))
          AND timestamp <= NOW() - make_interval(secs => $3::DOUBLE PRECISION)
        ORDER BY timestamp, id
        LIMIT $4
        "#,
        after.at,
        after.id,
        SETTLE.as_secs_f64(),
        limit
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
