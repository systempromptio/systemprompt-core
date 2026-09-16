//! AI-request analytics over the `ai_requests` table.
//!
//! [`RequestAnalyticsRepository`] reports token, cost, latency, and
//! cache-hit stats, per-model usage breakdowns, trend series, and a paged
//! request list, each optionally filtered by a model substring and the list
//! by user as well.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;

use crate::models::reporting::{
    ModelUsageRow, RequestListFilter, RequestListRow, RequestStatsRow, RequestTrendRow,
};

#[derive(Debug)]
pub struct RequestAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl RequestAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn get_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        model_filter: Option<&str>,
    ) -> Result<RequestStatsRow> {
        if let Some(model) = model_filter {
            let pattern = format!("%{model}%");
            sqlx::query_as!(
                RequestStatsRow,
                r#"
                SELECT
                    COUNT(*)::bigint as "total!",
                    SUM(tokens_used)::bigint as "total_tokens",
                    SUM(input_tokens)::bigint as "input_tokens",
                    SUM(output_tokens)::bigint as "output_tokens",
                    SUM(reasoning_tokens)::bigint as "reasoning_tokens",
                    SUM(cache_read_tokens)::bigint as "cache_read_tokens",
                    SUM(cache_creation_tokens)::bigint as "cache_creation_tokens",
                    SUM(cost_microdollars)::bigint as "cost",
                    AVG(latency_ms)::float8 as "avg_latency",
                    COUNT(*) FILTER (WHERE cache_hit = true)::bigint as "cache_hits!"
                FROM analytics_report_ai_requests
                WHERE created_at >= $1 AND created_at < $2
                  AND model ILIKE $3
                "#,
                start,
                end,
                pattern
            )
            .fetch_one(&*self.pool)
            .await
            .map_err(Into::into)
        } else {
            sqlx::query_as!(
                RequestStatsRow,
                r#"
                SELECT
                    COUNT(*)::bigint as "total!",
                    SUM(tokens_used)::bigint as "total_tokens",
                    SUM(input_tokens)::bigint as "input_tokens",
                    SUM(output_tokens)::bigint as "output_tokens",
                    SUM(reasoning_tokens)::bigint as "reasoning_tokens",
                    SUM(cache_read_tokens)::bigint as "cache_read_tokens",
                    SUM(cache_creation_tokens)::bigint as "cache_creation_tokens",
                    SUM(cost_microdollars)::bigint as "cost",
                    AVG(latency_ms)::float8 as "avg_latency",
                    COUNT(*) FILTER (WHERE cache_hit = true)::bigint as "cache_hits!"
                FROM analytics_report_ai_requests
                WHERE created_at >= $1 AND created_at < $2
                "#,
                start,
                end
            )
            .fetch_one(&*self.pool)
            .await
            .map_err(Into::into)
        }
    }

    pub async fn list_models(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ModelUsageRow>> {
        sqlx::query_as!(
            ModelUsageRow,
            r#"
            SELECT
                provider as "provider!",
                model as "model!",
                COUNT(*)::bigint as "request_count!",
                SUM(tokens_used)::bigint as "total_tokens",
                SUM(cost_microdollars)::bigint as "total_cost",
                AVG(latency_ms)::float8 as "avg_latency"
            FROM analytics_report_ai_requests
            WHERE created_at >= $1 AND created_at < $2
              AND provider IS NOT NULL AND model IS NOT NULL
            GROUP BY provider, model
            ORDER BY COUNT(*) DESC
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_requests_for_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<RequestTrendRow>> {
        sqlx::query_as!(
            RequestTrendRow,
            r#"
            SELECT
                created_at as "created_at!",
                tokens_used,
                cost_microdollars,
                latency_ms
            FROM analytics_report_ai_requests
            WHERE created_at >= $1 AND created_at < $2
            ORDER BY created_at
            "#,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn list_requests(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        filter: &RequestListFilter,
    ) -> Result<Vec<RequestListRow>> {
        let pattern = filter.model.as_deref().map(|model| format!("%{model}%"));
        let user = filter.user.as_ref().map(UserId::as_str);
        sqlx::query_as!(
            RequestListRow,
            r#"
            SELECT
                id as "id!",
                provider,
                model,
                input_tokens,
                output_tokens,
                cost_microdollars,
                latency_ms,
                cache_hit,
                created_at as "created_at!",
                status as "status!",
                error_message,
                user_id as "user_id!: UserId"
            FROM analytics_report_ai_requests
            WHERE created_at >= $1 AND created_at < $2
              AND ($3::text IS NULL OR model ILIKE $3)
              AND ($4::text IS NULL OR user_id = $4)
            ORDER BY created_at DESC, id DESC
            LIMIT $5 OFFSET $6
            "#,
            start,
            end,
            pattern,
            user,
            filter.limit,
            filter.offset
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }
}
