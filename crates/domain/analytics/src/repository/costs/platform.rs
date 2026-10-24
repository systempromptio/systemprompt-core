//! Platform-wide cost queries for `CostAnalyticsRepository`.
//!
//! Aggregates spend, tokens, and request counts across all users, with
//! breakdowns by model, provider, agent, and user and a trend series for the
//! platform cost dashboard. Sums and counts read the hourly rollup
//! (`report_ai_usage_hourly`), widening the window to whole UTC hours at its
//! start; the agent of a request is the one its task had when the request
//! was written; the billing-source split reads the rollup's personal-key
//! share. The per-user breakdown, which counts conversations, lives in
//! `conversations`; the trend series stays on `report_ai_requests` for its
//! window, being per request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::CostAnalyticsRepository;
use crate::Result;
use chrono::{DateTime, Utc};

use crate::models::reporting::{CostBreakdownRow, CostSummaryRow, CostTrendRow, PreviousCostRow};

impl CostAnalyticsRepository {
    pub async fn get_summary(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<CostSummaryRow> {
        sqlx::query_as!(
            CostSummaryRow,
            r#"
            SELECT
                COALESCE(SUM(request_count), 0)::bigint as "requests!",
                SUM(cost_microdollars)::bigint as "cost",
                SUM(tokens)::bigint as "tokens",
                SUM(reasoning_tokens)::bigint as "reasoning_tokens",
                SUM(cache_read_tokens)::bigint as "cache_read_tokens",
                SUM(cache_creation_tokens)::bigint as "cache_creation_tokens"
            FROM report_ai_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND traffic = 'production'
            "#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_previous_cost(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<PreviousCostRow> {
        sqlx::query_as!(
            PreviousCostRow,
            r#"
            SELECT SUM(cost_microdollars)::bigint as "cost"
            FROM report_ai_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND traffic = 'production'
            "#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_breakdown_by_model(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>> {
        sqlx::query_as!(
            CostBreakdownRow,
            r#"
            SELECT
                model as "name!",
                COALESCE(SUM(cost_microdollars), 0)::bigint as "cost!",
                SUM(request_count)::bigint as "requests!",
                COALESCE(SUM(tokens), 0)::bigint as "tokens!"
            FROM report_ai_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND traffic = 'production'
              AND model <> ''
            GROUP BY model
            ORDER BY SUM(cost_microdollars) DESC NULLS LAST
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

    pub async fn get_breakdown_by_provider(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>> {
        sqlx::query_as!(
            CostBreakdownRow,
            r#"
            SELECT
                provider as "name!",
                COALESCE(SUM(cost_microdollars), 0)::bigint as "cost!",
                SUM(request_count)::bigint as "requests!",
                COALESCE(SUM(tokens), 0)::bigint as "tokens!"
            FROM report_ai_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND traffic = 'production'
              AND provider <> ''
            GROUP BY provider
            ORDER BY SUM(cost_microdollars) DESC NULLS LAST
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

    pub async fn get_breakdown_by_billing_source(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>> {
        sqlx::query_as!(
            CostBreakdownRow,
            r#"
            WITH totals AS (
                SELECT
                    COALESCE(SUM(cost_microdollars), 0)::bigint AS cost,
                    COALESCE(SUM(request_count), 0)::bigint AS requests,
                    COALESCE(SUM(tokens), 0)::bigint AS tokens,
                    COALESCE(SUM(user_key_cost_microdollars), 0)::bigint AS key_cost,
                    COALESCE(SUM(user_key_request_count), 0)::bigint AS key_requests,
                    COALESCE(SUM(user_key_tokens), 0)::bigint AS key_tokens
                FROM report_ai_usage_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
                  AND traffic = 'production'
            )
            SELECT
                split.name as "name!",
                split.cost as "cost!",
                split.requests as "requests!",
                split.tokens as "tokens!"
            FROM totals t
            CROSS JOIN LATERAL (VALUES
                ('platform', t.cost - t.key_cost, t.requests - t.key_requests,
                 t.tokens - t.key_tokens),
                ('user_key', t.key_cost, t.key_requests, t.key_tokens)
            ) AS split(name, cost, requests, tokens)
            WHERE split.requests > 0
            ORDER BY split.cost DESC
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

    pub async fn get_breakdown_by_agent(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>> {
        sqlx::query_as!(
            CostBreakdownRow,
            r#"
            (
                SELECT
                    agent_name as "name!",
                    COALESCE(SUM(cost_microdollars), 0)::bigint as "cost!",
                    SUM(request_count)::bigint as "requests!",
                    COALESCE(SUM(tokens), 0)::bigint as "tokens!"
                FROM report_ai_usage_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
                  AND traffic = 'production'
                  AND agent_name <> ''
                GROUP BY agent_name
                ORDER BY SUM(cost_microdollars) DESC NULLS LAST
                LIMIT $3
            )
            UNION ALL
            (
                SELECT
                    'unattributed' as "name!",
                    COALESCE(SUM(cost_microdollars), 0)::bigint as "cost!",
                    COALESCE(SUM(request_count), 0)::bigint as "requests!",
                    COALESCE(SUM(tokens), 0)::bigint as "tokens!"
                FROM report_ai_usage_hourly
                WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
                  AND traffic = 'production'
                  AND agent_name = ''
                HAVING SUM(request_count) > 0
            )
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_costs_for_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<CostTrendRow>> {
        sqlx::query_as!(
            CostTrendRow,
            r#"
            SELECT
                created_at as "created_at!",
                cost_microdollars,
                tokens_used
            FROM report_ai_requests
            WHERE created_at >= $1 AND created_at < $2
              AND traffic = 'production'
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
