//! Platform-wide and per-user cost reads for [`AiUsageReportRepository`].
//!
//! Sums and counts read the hourly rollup (`report_ai_usage_hourly`),
//! widening the window to whole UTC hours at its start; the platform-wide
//! breakdowns are in `cost_breakdowns`. The trend series stays on
//! `report_ai_requests`, being per request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;
use systemprompt_traits::RepositoryError;
use systemprompt_traits::usage_reports::ai::{
    CostBreakdownRow, CostSummaryRow, CostTrendRow, PreviousCostRow,
};

use super::AiUsageReportRepository;

impl AiUsageReportRepository {
    pub async fn summary(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<CostSummaryRow, RepositoryError> {
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
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }

    pub async fn previous_cost(
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
              AND traffic = 'production'
            "#,
            start,
            end
        )
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }

    pub async fn cost_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<CostTrendRow>, RepositoryError> {
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
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }

    pub async fn summary_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<CostSummaryRow, RepositoryError> {
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
              AND traffic = 'production' AND user_id = $3
            "#,
            start,
            end,
            user_id as _
        )
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }

    pub async fn previous_cost_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<PreviousCostRow, RepositoryError> {
        sqlx::query_as!(
            PreviousCostRow,
            r#"
            SELECT SUM(cost_microdollars)::bigint as "cost"
            FROM report_ai_usage_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz, 'UTC') AND bucket < $2
              AND traffic = 'production' AND user_id = $3
            "#,
            start,
            end,
            user_id as _
        )
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }

    pub async fn breakdown_by_model_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>, RepositoryError> {
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
              AND traffic = 'production' AND user_id = $4
              AND model <> ''
            GROUP BY model
            ORDER BY SUM(cost_microdollars) DESC NULLS LAST
            LIMIT $3
            "#,
            start,
            end,
            limit,
            user_id as _
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(Into::into)
    }
}
