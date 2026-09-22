//! Projection bookkeeping: the singleton state row, the projector lock, the
//! rebuild-in-progress marker and the durable-queue lag behind it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgConnection, PgPool};

use crate::{AnalyticsError, Result};

const PROJECTOR_LOCK: i64 = 0x5350_414e_414c_5954;

/// Baseline state plus the pending durable facts still owed to the projection.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectionStatus {
    pub initialized: bool,
    pub generation: i64,
    pub rebuilt_at: Option<DateTime<Utc>>,
    pub rebuild_started_at: Option<DateTime<Utc>>,
    pub rebuild_heartbeat_at: Option<DateTime<Utc>>,
    pub rebuild_source: Option<String>,
    pub rebuild_rows: i64,
    pub pending_count: i64,
    pub oldest_pending_at: Option<DateTime<Utc>>,
    pub last_processed_at: Option<DateTime<Utc>>,
}

/// The state row as a rebuild sees it under the projector lock.
#[derive(Debug, Clone, Copy)]
pub struct RebuildState {
    pub generation: i64,
    pub initialized: bool,
    pub rebuild_started_at: Option<DateTime<Utc>>,
    pub rebuild_heartbeat_at: Option<DateTime<Utc>>,
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

pub async fn rebuild_state(connection: &mut PgConnection) -> Result<RebuildState> {
    Ok(sqlx::query_as!(
        RebuildState,
        "SELECT generation, initialized, rebuild_started_at, rebuild_heartbeat_at
         FROM analytics_projection_state WHERE singleton"
    )
    .fetch_one(connection)
    .await?)
}

/// Records a page of the running rebuild. The predicate is the fence: a
/// forced rebuild or a privacy compaction that moved the generation, or a
/// finished baseline, makes this write nothing and the caller stops.
pub async fn heartbeat_rebuild(
    connection: &mut PgConnection,
    generation: i64,
    source: &str,
    rows: i64,
) -> Result<()> {
    let result = sqlx::query!(
        "UPDATE analytics_projection_state
         SET rebuild_heartbeat_at = NOW(), rebuild_source = $2, rebuild_rows = rebuild_rows + $3
         WHERE singleton AND generation = $1 AND NOT initialized AND rebuild_started_at IS NOT NULL",
        generation,
        source,
        rows
    )
    .execute(connection)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AnalyticsError::rebuild_superseded());
    }
    Ok(())
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
            rebuild_started_at, rebuild_heartbeat_at, rebuild_source, rebuild_rows,
            (SELECT COUNT(*) FROM event_outbox WHERE consumer = $1 AND processed_at IS NULL) AS "pending_count!",
            (SELECT MIN(created_at) FROM event_outbox WHERE consumer = $1 AND processed_at IS NULL) AS oldest_pending_at,
            (SELECT MAX(processed_at) FROM event_outbox WHERE consumer = $1) AS last_processed_at
         FROM analytics_projection_state WHERE singleton"#,
        consumer
    )
    .fetch_one(pool)
    .await?)
}
