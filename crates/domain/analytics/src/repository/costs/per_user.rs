//! Per-user cost queries for `CostAnalyticsRepository`.
//!
//! Scopes spend and token reads to a single [`UserId`] for per-user billing
//! views. Spend, tokens and the per-model cost breakdown read the hourly
//! rollup (`report_ai_usage_hourly`), widening the window to whole UTC hours
//! at its start. The user's conversation reads live in `conversations`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::CostAnalyticsRepository;
use crate::Result;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;

use crate::models::reporting::{CostBreakdownRow, CostSummaryRow, PreviousCostRow};

impl CostAnalyticsRepository {
    pub async fn get_summary_for_user(
        &self,
        user_id: &UserId,
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
              AND traffic = 'production' AND user_id = $3
            "#,
            start,
            end,
            user_id.as_str()
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_previous_cost_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<PreviousCostRow> {
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
            user_id.as_str()
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_breakdown_by_model_for_user(
        &self,
        user_id: &UserId,
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
              AND traffic = 'production' AND user_id = $4
              AND model <> ''
            GROUP BY model
            ORDER BY SUM(cost_microdollars) DESC NULLS LAST
            LIMIT $3
            "#,
            start,
            end,
            limit,
            user_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }
}
