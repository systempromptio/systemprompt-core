//! Request reads for [`AiUsageReportRepository`]: token, cost, latency and
//! cache-hit stats, the per-model breakdown, trend series, the paged request
//! list, the overview's request and cost figures and the gateway sessions.
//!
//! Stats, the model breakdown and the overview figures read the hourly rollup
//! (`report_ai_usage_hourly`), widening the window to whole UTC hours at its
//! start; the rest are per-request reads of `report_ai_requests`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::RepositoryError;
use systemprompt_traits::usage_reports::ai::{
    GatewaySessionListRow, ModelUsageRow, PreviousCostRow, RequestListFilter, RequestListRow,
    RequestMetricsRow, RequestStatsRow, RequestTrendRow,
};

use super::AiUsageReportRepository;

impl AiUsageReportRepository {
    pub async fn request_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        model_filter: Option<&str>,
    ) -> Result<RequestStatsRow, RepositoryError> {
        if let Some(model) = model_filter {
            let pattern = format!("%{model}%");
            sqlx::query_as!(
                RequestStatsRow,
                r#"
                SELECT
                    COALESCE(SUM(request_count), 0)::bigint as "total!",
                    SUM(tokens)::bigint as "total_tokens",
                    SUM(input_tokens)::bigint as "input_tokens",
                    SUM(output_tokens)::bigint as "output_tokens",
                    SUM(reasoning_tokens)::bigint as "reasoning_tokens",
                    SUM(cache_read_tokens)::bigint as "cache_read_tokens",
                    SUM(cache_creation_tokens)::bigint as "cache_creation_tokens",
                    SUM(cost_microdollars)::bigint as "cost",
                    (SUM(latency_sum_ms)::numeric / NULLIF(SUM(latency_n), 0))::float8 as "avg_latency",
                    COALESCE(SUM(cache_hits), 0)::bigint as "cache_hits!"
                FROM report_ai_usage_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
                  AND model ILIKE $3
                "#,
                start,
                end,
                pattern
            )
            .fetch_one(self.pool.as_ref())
            .await
            .map_err(Into::into)
        } else {
            sqlx::query_as!(
                RequestStatsRow,
                r#"
                SELECT
                    COALESCE(SUM(request_count), 0)::bigint as "total!",
                    SUM(tokens)::bigint as "total_tokens",
                    SUM(input_tokens)::bigint as "input_tokens",
                    SUM(output_tokens)::bigint as "output_tokens",
                    SUM(reasoning_tokens)::bigint as "reasoning_tokens",
                    SUM(cache_read_tokens)::bigint as "cache_read_tokens",
                    SUM(cache_creation_tokens)::bigint as "cache_creation_tokens",
                    SUM(cost_microdollars)::bigint as "cost",
                    (SUM(latency_sum_ms)::numeric / NULLIF(SUM(latency_n), 0))::float8 as "avg_latency",
                    COALESCE(SUM(cache_hits), 0)::bigint as "cache_hits!"
                FROM report_ai_usage_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
                "#,
                start,
                end
            )
            .fetch_one(self.pool.as_ref())
            .await
            .map_err(Into::into)
        }
    }

    pub async fn model_usage(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ModelUsageRow>, RepositoryError> {
        sqlx::query_as!(
            ModelUsageRow,
            r#"
            SELECT
                provider as "provider!",
                model as "model!",
                SUM(request_count)::bigint as "request_count!",
                SUM(tokens)::bigint as "total_tokens",
                SUM(cost_microdollars)::bigint as "total_cost",
                (SUM(latency_sum_ms)::numeric / NULLIF(SUM(latency_n), 0))::float8 as "avg_latency"
            FROM report_ai_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND provider <> '' AND model <> ''
            GROUP BY provider, model
            ORDER BY SUM(request_count) DESC
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }

    pub async fn request_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<RequestTrendRow>, RepositoryError> {
        sqlx::query_as!(
            RequestTrendRow,
            r#"
            SELECT
                created_at as "created_at!",
                tokens_used,
                cost_microdollars,
                latency_ms
            FROM report_ai_requests
            WHERE created_at >= $1 AND created_at < $2
            ORDER BY created_at
            "#,
            start,
            end
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }

    pub async fn list_requests(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        filter: &RequestListFilter,
    ) -> Result<Vec<RequestListRow>, RepositoryError> {
        let pattern = filter.model.as_deref().map(|model| format!("%{model}%"));
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
                status as "status!: systemprompt_traits::RequestStatus",
                error_message,
                user_id as "user_id!: UserId"
            FROM report_ai_requests
            WHERE created_at >= $1 AND created_at < $2
              AND ($3::text IS NULL OR model ILIKE $3)
              AND ($4::text IS NULL OR user_id = $4)
            ORDER BY created_at DESC, id DESC
            LIMIT $5 OFFSET $6
            "#,
            start,
            end,
            pattern,
            filter.user as _,
            filter.limit,
            filter.offset
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }

    pub async fn request_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<RequestMetricsRow, RepositoryError> {
        sqlx::query_as!(
            RequestMetricsRow,
            r#"
            SELECT
                COALESCE(SUM(request_count), 0)::bigint as "total!",
                SUM(tokens)::bigint as "total_tokens",
                (SUM(latency_sum_ms)::numeric / NULLIF(SUM(latency_n), 0))::float8 as "avg_latency"
            FROM report_ai_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
            "#,
            start,
            end
        )
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }

    pub async fn overview_cost(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<PreviousCostRow, RepositoryError> {
        sqlx::query_as!(
            PreviousCostRow,
            r#"
            SELECT SUM(cost_microdollars)::bigint as "cost"
            FROM report_ai_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
            "#,
            start,
            end
        )
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }

    pub async fn list_gateway_sessions(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        user: Option<&UserId>,
    ) -> Result<Vec<GatewaySessionListRow>, RepositoryError> {
        sqlx::query_as!(
            GatewaySessionListRow,
            r#"
            SELECT
                ar.session_id as "session_id!: SessionId",
                MIN(ar.user_id) as "user_id!: UserId",
                COALESCE(SUM(ar.message_count), 0)::bigint as "message_count!",
                MIN(ar.created_at) as "created_at!",
                MAX(ar.created_at) as "updated_at!"
            FROM report_ai_requests ar
            WHERE ar.task_id IS NULL
              AND ar.session_id IS NOT NULL
              AND ar.created_at >= $1 AND ar.created_at < $2
              AND ($4::text IS NULL OR ar.user_id = $4)
            GROUP BY ar.session_id
            ORDER BY MAX(ar.created_at) DESC
            LIMIT $3
            "#,
            start,
            end,
            limit,
            user as _
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }
}
