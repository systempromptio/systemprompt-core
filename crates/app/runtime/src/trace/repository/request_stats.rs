//! AI-request aggregate queries: totals, per-provider and per-model rollups.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};

use super::{Result, TraceRepository};
use crate::trace::models::{AiRequestStats, ModelStatsRow, ProviderStatsRow};

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

impl TraceRepository {
    pub async fn get_ai_request_stats(
        &self,
        since: Option<DateTime<Utc>>,
    ) -> Result<AiRequestStats> {
        let totals = self.fetch_request_totals(since).await?;
        let provider_rows = self.fetch_provider_stats(since).await?;
        let model_rows = self.fetch_model_stats(since).await?;

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

    async fn fetch_request_totals(&self, since: Option<DateTime<Utc>>) -> Result<TotalRow> {
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
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    async fn fetch_provider_stats(&self, since: Option<DateTime<Utc>>) -> Result<Vec<ProviderRow>> {
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
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    async fn fetch_model_stats(&self, since: Option<DateTime<Utc>>) -> Result<Vec<ModelRow>> {
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
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }
}
