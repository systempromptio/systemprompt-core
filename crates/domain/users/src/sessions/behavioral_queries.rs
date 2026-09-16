//! Authoritative session signals for behavioral analysis.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_identifiers::SessionId;

use systemprompt_traits::session_store::SessionBehavioralData;

pub(super) async fn count_sessions_by_fingerprint(
    pool: &PgPool,
    fingerprint_hash: &str,
    window_hours: i64,
) -> Result<i64> {
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*)::BIGINT as "count!"
        FROM user_sessions
        WHERE fingerprint_hash = $1
          AND started_at > CURRENT_TIMESTAMP - make_interval(hours => $2)
        "#,
        fingerprint_hash,
        window_hours as i32
    )
    .fetch_one(pool)
    .await?;

    Ok(count)
}


pub(super) async fn get_session_for_behavioral_analysis(
    pool: &PgPool,
    session_id: &SessionId,
) -> Result<Option<SessionBehavioralData>> {
    let id = session_id.as_str();

    sqlx::query_as!(
        SessionBehavioralData,
        r#"
        SELECT
            session_id,
            fingerprint_hash,
            user_agent,
            request_count,
            started_at as "started_at!",
            last_activity_at as "last_activity_at!",
            landing_page,
            entry_url
        FROM user_sessions
        WHERE session_id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}


pub(super) async fn count_unique_ips_by_fingerprint(
    pool: &PgPool,
    fingerprint_hash: &str,
    window_days: i64,
) -> Result<i64> {
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(DISTINCT ip_address)::BIGINT as "count!"
        FROM user_sessions
        WHERE fingerprint_hash = $1
          AND ip_address IS NOT NULL
          AND started_at > CURRENT_TIMESTAMP - make_interval(days => $2)
        "#,
        fingerprint_hash,
        window_days as i32
    )
    .fetch_one(pool)
    .await?;

    Ok(count)
}


pub(super) async fn get_session_starts_by_fingerprint(
    pool: &PgPool,
    fingerprint_hash: &str,
    window_days: i64,
) -> Result<Vec<DateTime<Utc>>> {
    let timestamps = sqlx::query_scalar!(
        r#"
        SELECT started_at as "started_at!"
        FROM user_sessions
        WHERE fingerprint_hash = $1
          AND started_at > CURRENT_TIMESTAMP - make_interval(days => $2)
        ORDER BY started_at ASC
        "#,
        fingerprint_hash,
        window_days as i32
    )
    .fetch_all(pool)
    .await?;

    Ok(timestamps)
}

pub(super) async fn get_session_velocity(
    pool: &PgPool,
    session_id: &SessionId,
) -> Result<(Option<i64>, Option<i64>)> {
    let id = session_id.as_str();

    let row = sqlx::query!(
        r#"
        SELECT
            request_count::BIGINT as request_count,
            EXTRACT(EPOCH FROM (last_activity_at - started_at))::BIGINT as duration_seconds
        FROM user_sessions
        WHERE session_id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map_or((None, None), |r| (r.request_count, r.duration_seconds)))
}
