use anyhow::Result;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;

use super::models::{AiRequestSummary, TraceEvent};

pub use super::step_queries::{
    fetch_execution_step_events, fetch_execution_step_summary, fetch_mcp_execution_events,
    fetch_mcp_execution_summary, fetch_task_id_for_trace,
};

pub async fn fetch_log_events(pool: &Arc<PgPool>, trace_id: &str) -> Result<Vec<TraceEvent>> {
    let rows = sqlx::query!(
        r#"
        SELECT
            timestamp,
            level as type,
            CONCAT(module, ': ', message) as details,
            user_id,
            session_id,
            task_id,
            context_id,
            metadata
        FROM logs
        WHERE trace_id = $1
        ORDER BY timestamp ASC
        "#,
        trace_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| TraceEvent {
            event_type: row.r#type,
            timestamp: row.timestamp,
            details: row.details.unwrap_or_else(String::new),
            user_id: row.user_id,
            session_id: row.session_id,
            task_id: row.task_id,
            context_id: row.context_id,
            metadata: row.metadata,
        })
        .collect())
}

pub async fn fetch_ai_request_summary(
    pool: &Arc<PgPool>,
    trace_id: &str,
) -> Result<AiRequestSummary> {
    let row = sqlx::query!(
        r#"
        SELECT
            COALESCE(SUM(cost_microdollars), 0)::bigint as total_cost_microdollars,
            COALESCE(SUM(COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)), 0)::bigint as total_tokens,
            COALESCE(SUM(input_tokens), 0)::bigint as total_input_tokens,
            COALESCE(SUM(output_tokens), 0)::bigint as total_output_tokens,
            COUNT(*)::bigint as request_count,
            COALESCE(SUM(latency_ms), 0)::bigint as total_latency_ms
        FROM ai_requests
        WHERE trace_id = $1
        "#,
        trace_id
    )
    .fetch_one(&**pool)
    .await?;

    Ok(AiRequestSummary {
        total_cost_microdollars: row.total_cost_microdollars.unwrap_or(0),
        total_tokens: row.total_tokens.unwrap_or(0),
        total_input_tokens: row.total_input_tokens.unwrap_or(0),
        total_output_tokens: row.total_output_tokens.unwrap_or(0),
        request_count: row.request_count.unwrap_or(0),
        total_latency_ms: row.total_latency_ms.unwrap_or(0),
    })
}

pub async fn fetch_ai_request_events(
    pool: &Arc<PgPool>,
    trace_id: &str,
) -> Result<Vec<TraceEvent>> {
    let rows = sqlx::query!(
        r#"
        SELECT
            created_at as timestamp,
            provider,
            model,
            input_tokens,
            output_tokens,
            cost_microdollars,
            latency_ms,
            status,
            user_id,
            session_id,
            task_id,
            context_id
        FROM ai_requests
        WHERE trace_id = $1
        ORDER BY created_at ASC
        "#,
        trace_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let details = format!(
                "{}/{}: {} (in:{}, out:{}, {}ms)",
                row.provider,
                row.model,
                row.status,
                row.input_tokens.unwrap_or(0),
                row.output_tokens.unwrap_or(0),
                row.latency_ms.unwrap_or(0)
            );

            let metadata = json!({
                "cost_microdollars": row.cost_microdollars,
                "latency_ms": row.latency_ms,
                "input_tokens": row.input_tokens,
                "output_tokens": row.output_tokens,
                "tokens_used": row.input_tokens.unwrap_or(0) + row.output_tokens.unwrap_or(0),
                "provider": row.provider,
                "model": row.model
            });

            TraceEvent {
                event_type: "AI".to_string(),
                timestamp: row.timestamp,
                details,
                user_id: Some(row.user_id),
                session_id: row.session_id,
                task_id: row.task_id,
                context_id: row.context_id,
                metadata: Some(metadata.to_string()),
            }
        })
        .collect())
}
