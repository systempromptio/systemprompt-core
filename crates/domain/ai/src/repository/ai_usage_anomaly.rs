//! Per-user hourly usage profile for the usage-anomaly scan.
//!
//! For one closed window `[since, until)` it returns, per user active in that
//! window, the window's completed request count and cost beside the user's
//! trailing seven-day hourly average ending at `since`. Synthetic and
//! non-completed rows are excluded from both sides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_traits::RepositoryError;

const BASELINE_HOURS: f64 = 7.0 * 24.0;

/// One user's usage in the scanned window against their hourly baseline.
#[derive(Debug, Clone, PartialEq)]
pub struct HourlyUsageProfile {
    pub user_id: UserId,
    pub observed_requests: i64,
    pub observed_cost_microdollars: i64,
    pub baseline_requests_per_hour: f64,
    pub baseline_cost_per_hour: f64,
}

#[must_use]
#[derive(Debug, Clone)]
pub struct AiUsageAnomalyRepository {
    pool: Arc<PgPool>,
}

impl AiUsageAnomalyRepository {
    pub fn new(db: &DbPool) -> Self {
        Self { pool: db.pool() }
    }

    pub async fn hourly_profile(
        &self,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<HourlyUsageProfile>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            WITH observed AS (
                SELECT user_id, COUNT(*)::BIGINT AS requests,
                       COALESCE(SUM(cost_microdollars), 0)::BIGINT AS cost
                FROM ai_requests
                WHERE created_at >= $1 AND created_at < $2
                  AND status = 'completed' AND synthetic = FALSE
                GROUP BY user_id
            ),
            baseline AS (
                SELECT user_id, COUNT(*)::BIGINT AS requests,
                       COALESCE(SUM(cost_microdollars), 0)::BIGINT AS cost
                FROM ai_requests
                WHERE created_at >= $1 - INTERVAL '7 days' AND created_at < $1
                  AND status = 'completed' AND synthetic = FALSE
                  AND user_id IN (SELECT user_id FROM observed)
                GROUP BY user_id
            )
            SELECT o.user_id AS "user_id!",
                   o.requests AS "observed_requests!",
                   o.cost AS "observed_cost!",
                   COALESCE(b.requests, 0)::BIGINT AS "baseline_requests!",
                   COALESCE(b.cost, 0)::BIGINT AS "baseline_cost!"
            FROM observed o
            LEFT JOIN baseline b ON b.user_id = o.user_id
            ORDER BY o.user_id
            "#,
            since,
            until,
        )
        .fetch_all(self.pool.as_ref())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| HourlyUsageProfile {
                user_id: UserId::new(row.user_id),
                observed_requests: row.observed_requests,
                observed_cost_microdollars: row.observed_cost,
                baseline_requests_per_hour: row.baseline_requests as f64 / BASELINE_HOURS,
                baseline_cost_per_hour: row.baseline_cost as f64 / BASELINE_HOURS,
            })
            .collect())
    }
}
