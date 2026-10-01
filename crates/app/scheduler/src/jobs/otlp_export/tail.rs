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

use sqlx::PgPool;
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_identifiers::{
    AiToolCallId, ClientId, ContextId, GatewayConversationId, InstanceId, McpExecutionId,
    McpServerId, McpToolName, PluginId, ProviderRequestId, SessionId, TraceId, UserId,
};
use systemprompt_traits::RepositoryError;

use super::records::{GovernanceRow, LedgerRow, LogRow, RequestRow};
use super::state::Watermark;
use crate::error::{SchedulerError, SchedulerResult};

pub const SETTLE: Duration = Duration::from_secs(5);

pub const BATCH_ROWS: i64 = 500;

fn decode_error(context: &str, source: IdValidationError) -> SchedulerError {
    SchedulerError::Repository(RepositoryError::Decode {
        context: context.to_owned(),
        source: Box::new(source),
    })
}

pub(super) async fn list_requests_after(
    pool: &PgPool,
    after: &Watermark,
    limit: i64,
) -> SchedulerResult<Vec<RequestRow>> {
    let rows = sqlx::query!(
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
    Ok(rows
        .into_iter()
        .map(|row| RequestRow {
            id: row.id,
            request_id: row.request_id,
            user_id: row.user_id,
            session_id: row.session_id,
            context_id: row.context_id,
            trace_id: row.trace_id,
            provider: row.provider,
            served_provider: row.served_provider,
            model: row.model,
            requested_model: row.requested_model,
            route_match: row.route_match,
            input_tokens: row.input_tokens,
            output_tokens: row.output_tokens,
            cache_read_tokens: row.cache_read_tokens,
            cache_creation_tokens: row.cache_creation_tokens,
            cost_microdollars: row.cost_microdollars,
            latency_ms: row.latency_ms,
            upstream_latency_ms: row.upstream_latency_ms,
            finish_reason: row.finish_reason,
            status: row.status,
            error_message: row.error_message,
            client_kind: row.client_kind,
            wire_protocol: row.wire_protocol,
            request_kind: row.request_kind,
            actor_kind: row.actor_kind,
            actor_id: row.actor_id,
            instance_id: row.instance_id.map(InstanceId::new),
            created_at: row.created_at,
            completed_at: row.completed_at,
        })
        .collect())
}

pub(super) async fn list_ledger_for_requests(
    pool: &PgPool,
    request_ids: &[String],
) -> SchedulerResult<Vec<LedgerRow>> {
    let rows = sqlx::query!(
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
    Ok(rows
        .into_iter()
        .map(|row| LedgerRow {
            ai_tool_call_id: row.ai_tool_call_id.map(AiToolCallId::new),
            request_id: row.request_id,
            mcp_execution_id: row.mcp_execution_id.map(McpExecutionId::new),
            tool_name: row.tool_name.map(McpToolName::new),
            server_name: row.server_name.map(McpServerId::new),
            intended_at: row.intended_at,
            executed_at: row.executed_at,
            completed_at: row.completed_at,
            execution_time_ms: row.execution_time_ms,
            execution_status: row.execution_status,
            error_message: row.error_message,
            source: row.source,
            state: row.state,
            is_error: row.is_error,
            artifact_type: row.artifact_type,
            payload_bytes: row.payload_bytes,
            secret_redactions: row.secret_redactions,
            occurred_at: row.occurred_at,
        })
        .collect())
}

pub(super) async fn list_governance_for_traces(
    pool: &PgPool,
    trace_ids: &[String],
) -> SchedulerResult<Vec<GovernanceRow>> {
    let rows = sqlx::query!(
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
    Ok(rows
        .into_iter()
        .map(|row| GovernanceRow {
            id: row.id,
            trace_id: row.trace_id,
            tool_name: row.tool_name,
            decision: row.decision,
            policy: row.policy,
            reason: row.reason,
            plugin_id: row.plugin_id.map(PluginId::new),
            actor_kind: row.actor_kind,
            actor_id: row.actor_id,
            tool_use_id: row.tool_use_id,
            created_at: row.created_at,
        })
        .collect())
}

pub(super) async fn list_logs_after(
    pool: &PgPool,
    after: &Watermark,
    limit: i64,
) -> SchedulerResult<Vec<LogRow>> {
    let rows = sqlx::query!(
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
    rows.into_iter()
        .map(|row| {
            Ok(LogRow {
                provider_request_id: row
                    .provider_request_id
                    .map(ProviderRequestId::try_new)
                    .transpose()
                    .map_err(|e| decode_error("logs.provider_request_id", e))?,
                gateway_conversation_id: row
                    .gateway_conversation_id
                    .map(GatewayConversationId::try_new)
                    .transpose()
                    .map_err(|e| decode_error("logs.gateway_conversation_id", e))?,
                id: row.id,
                timestamp: row.timestamp,
                level: row.level,
                module: row.module,
                message: row.message,
                metadata: row.metadata,
                user_id: row.user_id,
                session_id: row.session_id,
                trace_id: row.trace_id,
                context_id: row.context_id,
                client_id: row.client_id,
                instance_id: row.instance_id.map(InstanceId::new),
            })
        })
        .collect()
}
