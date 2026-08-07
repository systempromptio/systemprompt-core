//! AI-request listing and aggregate queries (totals, per-provider, per-model).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::LoggingError;
pub(super) type Result<T> = std::result::Result<T, LoggingError>;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;

use systemprompt_identifiers::{AiRequestId, TraceId, UserId};

use super::models::{
    AiRequestDetail, AiRequestFilter, AiRequestListItem, AiRequestStats, ModelStatsRow,
    ProviderStatsRow,
};

struct ListRow {
    id: AiRequestId,
    created_at: DateTime<Utc>,
    trace_id: Option<String>,
    user_id: UserId,
    actor_kind: String,
    actor_id: String,
    provider: Option<String>,
    model: Option<String>,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    cache_read_tokens: Option<i32>,
    cache_creation_tokens: Option<i32>,
    cost_microdollars: i64,
    latency_ms: Option<i32>,
    status: String,
}

struct TotalRow {
    request_count: Option<i64>,
    total_input_tokens: Option<i64>,
    total_output_tokens: Option<i64>,
    total_cost_microdollars: Option<i64>,
    avg_latency_ms: Option<i64>,
}

struct ProviderRow {
    provider: String,
    request_count: Option<i64>,
    total_tokens: Option<i64>,
    total_cost_microdollars: Option<i64>,
    avg_latency_ms: Option<i64>,
}

struct ModelRow {
    model: String,
    provider: String,
    request_count: Option<i64>,
    total_tokens: Option<i64>,
    total_cost_microdollars: Option<i64>,
    avg_latency_ms: Option<i64>,
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
            provider,
            model,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            cost_microdollars as "cost_microdollars!",
            latency_ms,
            status as "status!"
        FROM ai_requests
        WHERE ($1::timestamptz IS NULL OR created_at >= $1)
          AND ($2::text IS NULL OR model ILIKE $2)
          AND ($3::text IS NULL OR provider ILIKE $3)
          AND ($4::text IS NULL OR user_id = $4)
        ORDER BY created_at DESC
        LIMIT $5
        "#,
        since,
        model,
        provider,
        user,
        limit
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
            provider: r.provider,
            model: r.model,
            input_tokens: r.input_tokens,
            output_tokens: r.output_tokens,
            cache_read_tokens: r.cache_read_tokens,
            cache_creation_tokens: r.cache_creation_tokens,
            cost_microdollars: r.cost_microdollars,
            latency_ms: r.latency_ms,
            status: r.status,
        })
        .collect())
}

pub(super) async fn get_ai_request_stats(
    pool: &Arc<PgPool>,
    since: Option<DateTime<Utc>>,
) -> Result<AiRequestStats> {
    let totals = fetch_request_totals(pool, since).await?;
    let provider_rows = fetch_provider_stats(pool, since).await?;
    let model_rows = fetch_model_stats(pool, since).await?;

    Ok(AiRequestStats {
        total_requests: totals.request_count.unwrap_or(0),
        total_input_tokens: totals.total_input_tokens.unwrap_or(0),
        total_output_tokens: totals.total_output_tokens.unwrap_or(0),
        total_cost_microdollars: totals.total_cost_microdollars.unwrap_or(0),
        avg_latency_ms: totals.avg_latency_ms.unwrap_or(0),
        by_provider: provider_rows
            .into_iter()
            .map(|r| ProviderStatsRow {
                provider: r.provider,
                request_count: r.request_count.unwrap_or(0),
                total_tokens: r.total_tokens.unwrap_or(0),
                total_cost_microdollars: r.total_cost_microdollars.unwrap_or(0),
                avg_latency_ms: r.avg_latency_ms.unwrap_or(0),
            })
            .collect(),
        by_model: model_rows
            .into_iter()
            .map(|r| ModelStatsRow {
                model: r.model,
                provider: r.provider,
                request_count: r.request_count.unwrap_or(0),
                total_tokens: r.total_tokens.unwrap_or(0),
                total_cost_microdollars: r.total_cost_microdollars.unwrap_or(0),
                avg_latency_ms: r.avg_latency_ms.unwrap_or(0),
            })
            .collect(),
    })
}

async fn fetch_request_totals(
    pool: &Arc<PgPool>,
    since: Option<DateTime<Utc>>,
) -> Result<TotalRow> {
    sqlx::query_as!(
        TotalRow,
        r#"
        SELECT
            COUNT(*) as "request_count",
            COALESCE(SUM(input_tokens), 0) as "total_input_tokens",
            COALESCE(SUM(output_tokens), 0) as "total_output_tokens",
            COALESCE(SUM(cost_microdollars), 0)::bigint as "total_cost_microdollars",
            COALESCE(AVG(latency_ms), 0)::bigint as "avg_latency_ms"
        FROM ai_requests
        WHERE ($1::timestamptz IS NULL OR created_at >= $1)
        "#,
        since
    )
    .fetch_one(&**pool)
    .await
    .map_err(Into::into)
}

async fn fetch_provider_stats(
    pool: &Arc<PgPool>,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<ProviderRow>> {
    sqlx::query_as!(
        ProviderRow,
        r#"
        SELECT
            provider as "provider!",
            COUNT(*) as "request_count",
            COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0) as "total_tokens",
            COALESCE(SUM(cost_microdollars), 0)::bigint as "total_cost_microdollars",
            COALESCE(AVG(latency_ms), 0)::bigint as "avg_latency_ms"
        FROM ai_requests
        WHERE ($1::timestamptz IS NULL OR created_at >= $1)
          AND provider IS NOT NULL
        GROUP BY provider
        ORDER BY request_count DESC
        "#,
        since
    )
    .fetch_all(&**pool)
    .await
    .map_err(Into::into)
}

async fn fetch_model_stats(
    pool: &Arc<PgPool>,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<ModelRow>> {
    sqlx::query_as!(
        ModelRow,
        r#"
        SELECT
            model as "model!",
            provider as "provider!",
            COUNT(*) as "request_count",
            COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0) as "total_tokens",
            COALESCE(SUM(cost_microdollars), 0)::bigint as "total_cost_microdollars",
            COALESCE(AVG(latency_ms), 0)::bigint as "avg_latency_ms"
        FROM ai_requests
        WHERE ($1::timestamptz IS NULL OR created_at >= $1)
          AND model IS NOT NULL AND provider IS NOT NULL
        GROUP BY model, provider
        ORDER BY request_count DESC
        LIMIT 10
        "#,
        since
    )
    .fetch_all(&**pool)
    .await
    .map_err(Into::into)
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
