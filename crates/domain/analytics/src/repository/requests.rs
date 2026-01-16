
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_core_database::DbPool;

use crate::models::cli::{ModelUsageRow, RequestStatsRow, RequestTrendRow};

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
            let pattern = format!("%{}%", model);
            sqlx::query_as!(
                RequestStatsRow,
                r#"
                SELECT
                    COUNT(*)::bigint as "total!",
                    SUM(tokens_used)::bigint as "total_tokens",
                    SUM(input_tokens)::bigint as "input_tokens",
                    SUM(output_tokens)::bigint as "output_tokens",
                    SUM(cost_cents)::bigint as "cost",
                    AVG(latency_ms)::float8 as "avg_latency",
                    COUNT(*) FILTER (WHERE cache_hit = true)::bigint as "cache_hits!"
                FROM ai_requests
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
                    SUM(cost_cents)::bigint as "cost",
                    AVG(latency_ms)::float8 as "avg_latency",
                    COUNT(*) FILTER (WHERE cache_hit = true)::bigint as "cache_hits!"
                FROM ai_requests
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
                SUM(cost_cents)::bigint as "total_cost",
                AVG(latency_ms)::float8 as "avg_latency"
            FROM ai_requests
            WHERE created_at >= $1 AND created_at < $2
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
                cost_cents,
                latency_ms
            FROM ai_requests
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
}
