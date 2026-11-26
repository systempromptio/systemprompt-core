//! The session counts of the analytics overview.
//!
//! [`OverviewAnalyticsRepository`] reads the overview's session figures from
//! the analytics-owned traffic views. The conversation, agent, request, tool
//! and cost figures belong to the members that own those views and are read
//! through their report ports (`systemprompt_traits::usage_reports`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug)]
pub struct OverviewAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl OverviewAnalyticsRepository {
    pub fn new(db: &DbPool) -> Self {
        let pool = db.pool();
        Self { pool }
    }

    pub async fn get_active_session_count(&self, since: DateTime<Utc>) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*)::bigint as "count!"
            FROM report_clean_traffic
            WHERE ended_at IS NULL
              AND last_activity_at >= $1            "#,
            since
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(count)
    }

    pub async fn get_total_session_count(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint as "count!" FROM report_clean_traffic WHERE started_at >= $1 AND started_at < $2"#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(count)
    }
}
