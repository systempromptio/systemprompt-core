//! AI-request listing and detail queries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::TraceError;
pub(super) type Result<T> = std::result::Result<T, TraceError>;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;

use systemprompt_identifiers::{AiRequestId, TraceId, UserId};

use super::models::{AiRequestDetail, AiRequestFilter, AiRequestListItem};

struct ListRow {
    id: AiRequestId,
    created_at: DateTime<Utc>,
    trace_id: Option<String>,
    user_id: UserId,
    actor_kind: String,
    actor_id: String,
    client_kind: String,
    provider: Option<String>,
    model: Option<String>,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    cache_read_tokens: Option<i32>,
    cache_creation_tokens: Option<i32>,
    reasoning_tokens: Option<i32>,
    cost_microdollars: i64,
    latency_ms: Option<i32>,
    status: String,
}

struct DetailRow {
    id: AiRequestId,
    user_id: UserId,
    actor_kind: String,
    actor_id: String,
    provider: Option<String>,
    model: Option<String>,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    cost_microdollars: i64,
    latency_ms: Option<i32>,
    status: String,
    error_message: Option<String>,
}

pub(super) async fn list_ai_requests(
    pool: &Arc<PgPool>,
    filter: &AiRequestFilter,
) -> Result<Vec<AiRequestListItem>> {
    let since = filter.since;
    let until = filter.until;
    let before_at = filter.before.as_ref().map(|c| c.created_at);
    let before_id = filter.before.as_ref().map(|c| c.id.as_str());
    let model = filter.model.as_deref();
    let provider = filter.provider.as_deref();
    let user = filter.user.as_deref();
    let limit = filter.limit;
    let rows = sqlx::query_as!(
        ListRow,
        r#"
        SELECT
            id as "id!: AiRequestId",
            created_at as "created_at!",
            trace_id,
            user_id as "user_id!: UserId",
            actor_kind as "actor_kind!",
            actor_id as "actor_id!",
            client_kind as "client_kind!",
            provider, model,
            input_tokens, output_tokens,
            cache_read_tokens, cache_creation_tokens, reasoning_tokens,
            cost_microdollars as "cost_microdollars!",
            latency_ms,
            status as "status!"
        FROM ai_requests
        WHERE ($1::timestamptz IS NULL OR created_at >= $1)
          AND ($2::text IS NULL OR model ILIKE $2)
          AND ($3::text IS NULL OR provider ILIKE $3)
          AND ($4::text IS NULL OR user_id = $4)
          AND ($6::timestamptz IS NULL OR created_at < $6)
          AND ($7::timestamptz IS NULL OR (created_at, id) < ($7, $8))
        ORDER BY created_at DESC, id DESC
        LIMIT $5
        "#,
        since,
        model,
        provider,
        user,
        limit,
        until,
        before_at,
        before_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| AiRequestListItem {
            id: r.id,
            created_at: r.created_at,
            trace_id: r.trace_id.map(TraceId::new),
            user_id: r.user_id,
            actor_kind: r.actor_kind,
            actor_id: r.actor_id,
            client_kind: r.client_kind,
            provider: r.provider,
            model: r.model,
            input_tokens: r.input_tokens,
            output_tokens: r.output_tokens,
            cache_read_tokens: r.cache_read_tokens,
            cache_creation_tokens: r.cache_creation_tokens,
            reasoning_tokens: r.reasoning_tokens,
            cost_microdollars: r.cost_microdollars,
            latency_ms: r.latency_ms,
            status: r.status,
        })
        .collect())
}

pub(super) async fn find_ai_request_detail(
    pool: &Arc<PgPool>,
    id: &str,
) -> Result<Option<AiRequestDetail>> {
    let partial = format!("{id}%");
    let row = sqlx::query_as!(
        DetailRow,
        r#"
        SELECT
            id as "id!: AiRequestId",
            user_id as "user_id!: UserId",
            actor_kind as "actor_kind!",
            actor_id as "actor_id!",
            provider,
            model,
            input_tokens,
            output_tokens,
            cost_microdollars as "cost_microdollars!",
            latency_ms,
            status as "status!",
            error_message
        FROM ai_requests
        WHERE id = $1 OR id LIKE $2
        LIMIT 1
        "#,
        id,
        partial
    )
    .fetch_optional(&**pool)
    .await?;

    Ok(row.map(|r| AiRequestDetail {
        id: r.id,
        user_id: r.user_id,
        actor_kind: r.actor_kind,
        actor_id: r.actor_id,
        provider: r.provider,
        model: r.model,
        input_tokens: r.input_tokens,
        output_tokens: r.output_tokens,
        cost_microdollars: r.cost_microdollars,
        latency_ms: r.latency_ms,
        status: r.status,
        error_message: r.error_message,
    }))
}
