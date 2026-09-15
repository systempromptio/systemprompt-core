//! Reporting bootstrap state and durable queue lag.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::Serialize;
use systemprompt_analytics::AnalyticsError;
use systemprompt_analytics::projection::REPORTING_CONSUMER;
use systemprompt_database::DbPool;

use crate::RuntimeResult;

#[derive(Debug, Clone, Copy, Serialize, sqlx::FromRow)]
pub struct ReportingStatus {
    pub initialized: bool,
    pub generation: i64,
    pub rebuilt_at: Option<DateTime<Utc>>,
    pub pending_count: i64,
    pub oldest_pending_at: Option<DateTime<Utc>>,
    pub last_processed_at: Option<DateTime<Utc>>,
}

pub async fn status(db: &DbPool) -> RuntimeResult<ReportingStatus> {
    let pool = db.write_pool_arc()?;
    from_pool(&pool).await.map_err(Into::into)
}

pub(super) async fn from_pool(pool: &sqlx::PgPool) -> Result<ReportingStatus, AnalyticsError> {
    sqlx::query_as(
        "SELECT initialized, generation, rebuilt_at,
            (SELECT COUNT(*) FROM event_outbox WHERE consumer = $1 AND processed_at IS NULL) AS pending_count,
            (SELECT MIN(created_at) FROM event_outbox WHERE consumer = $1 AND processed_at IS NULL) AS oldest_pending_at,
            (SELECT MAX(processed_at) FROM event_outbox WHERE consumer = $1) AS last_processed_at
         FROM analytics_projection_state WHERE singleton",
    )
    .bind(REPORTING_CONSUMER)
    .fetch_one(pool)
    .await
    .map_err(AnalyticsError::from)
}
