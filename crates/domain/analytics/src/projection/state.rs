//! Projection bookkeeping: the singleton state row, the projector lock and
//! the durable-queue lag behind it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgConnection, PgPool};

use crate::Result;

const PROJECTOR_LOCK: i64 = 0x5350_414e_414c_5954;

/// Baseline state plus the pending durable facts still owed to the projection.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ProjectionStatus {
    pub initialized: bool,
    pub generation: i64,
    pub rebuilt_at: Option<DateTime<Utc>>,
    pub pending_count: i64,
    pub oldest_pending_at: Option<DateTime<Utc>>,
    pub last_processed_at: Option<DateTime<Utc>>,
}

pub async fn lock_projector(connection: &mut PgConnection) -> Result<()> {
    sqlx::query!("SELECT pg_advisory_xact_lock($1)", PROJECTOR_LOCK)
        .fetch_one(connection)
        .await?;
    Ok(())
}

pub async fn lock_user_deletion(connection: &mut PgConnection) -> Result<()> {
    sqlx::query_scalar!(r#"SELECT public.lock_user_deletion_for_retention() AS "locked!""#)
        .fetch_one(connection)
        .await?;
    Ok(())
}

pub async fn is_initialized(connection: &mut PgConnection) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT initialized AS "initialized!" FROM analytics_projection_state WHERE singleton"#
    )
    .fetch_one(connection)
    .await?)
}

pub async fn next_cutoff_revision(connection: &mut PgConnection) -> Result<i64> {
    Ok(
        sqlx::query_scalar!(r#"SELECT nextval('event_outbox_reporting_revision') AS "cutoff!""#)
            .fetch_one(connection)
            .await?,
    )
}

pub async fn status(pool: &PgPool, consumer: &str) -> Result<ProjectionStatus> {
    Ok(sqlx::query_as!(
        ProjectionStatus,
        r#"SELECT initialized, generation, rebuilt_at,
            (SELECT COUNT(*) FROM event_outbox WHERE consumer = $1 AND processed_at IS NULL) AS "pending_count!",
            (SELECT MIN(created_at) FROM event_outbox WHERE consumer = $1 AND processed_at IS NULL) AS oldest_pending_at,
            (SELECT MAX(processed_at) FROM event_outbox WHERE consumer = $1) AS last_processed_at
         FROM analytics_projection_state WHERE singleton"#,
        consumer
    )
    .fetch_one(pool)
    .await?)
}
