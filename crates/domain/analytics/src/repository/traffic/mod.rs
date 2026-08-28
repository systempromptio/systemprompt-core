//! Traffic-source, geography, device, and bot analytics.
//!
//! [`TrafficAnalyticsRepository`] reads `user_sessions` to break sessions
//! down by referrer source, country, and device, and to classify human
//! versus bot traffic (including a user-agent-driven bot taxonomy). An
//! `engaged_only` flag restricts the human-facing breakdowns to sessions with
//! a landing page and at least one request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod audience;
mod pages;

use crate::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

use crate::models::reporting::TrafficSourceRow;

#[derive(Debug, Clone, Copy)]
pub struct PageQuery<'a> {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub limit: i64,
    pub engaged_only: bool,
    pub referrer: Option<&'a str>,
    pub path_prefix: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct NavigationQuery<'a> {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub limit: i64,
    pub path_prefix: Option<&'a str>,
    pub internal_only: bool,
}

#[derive(Debug)]
pub struct TrafficAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl TrafficAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn get_sources(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        engaged_only: bool,
    ) -> Result<Vec<TrafficSourceRow>> {
        if engaged_only {
            sqlx::query_as!(
                TrafficSourceRow,
                r#"
                SELECT
                    COALESCE(referrer_source, 'direct') as "source",
                    COUNT(*)::bigint as "count!"
                FROM v_engaged_traffic
                WHERE started_at >= $1 AND started_at < $2
                GROUP BY referrer_source
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
        } else {
            sqlx::query_as!(
                TrafficSourceRow,
                r#"
                SELECT
                    COALESCE(referrer_source, 'direct') as "source",
                    COUNT(*)::bigint as "count!"
                FROM v_clean_traffic
                WHERE started_at >= $1 AND started_at < $2
                GROUP BY referrer_source
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
    }
}
