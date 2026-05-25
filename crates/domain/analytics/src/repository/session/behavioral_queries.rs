//! Behavioural-detector / fingerprint-windowed read queries against
//! `user_sessions`, `analytics_events`, and `engagement_events`. Split from
//! `queries.rs` to keep each module under 300 lines.

use crate::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_identifiers::SessionId;

use super::types::SessionBehavioralData;

pub(crate) async fn count_sessions_by_fingerprint(
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

pub(crate) async fn get_endpoint_sequence(pool: &PgPool, session_id: &SessionId) -> Result<Vec<String>> {
    let id = session_id.as_str();

    let endpoints = sqlx::query_scalar!(
        r#"
        SELECT endpoint
        FROM analytics_events
        WHERE session_id = $1
          AND event_type = 'page_view'
        ORDER BY timestamp ASC
        "#,
        id
    )
    .fetch_all(pool)
    .await?;

    Ok(endpoints.into_iter().flatten().collect())
}

pub(crate) async fn get_request_timestamps(
    pool: &PgPool,
    session_id: &SessionId,
) -> Result<Vec<DateTime<Utc>>> {
    let id = session_id.as_str();

    let timestamps = sqlx::query_scalar!(
        r#"
        SELECT timestamp as "timestamp!"
        FROM analytics_events
        WHERE session_id = $1
        ORDER BY timestamp ASC
        "#,
        id
    )
    .fetch_all(pool)
    .await?;

    Ok(timestamps)
}

pub(crate) async fn get_session_for_behavioral_analysis(
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

pub(crate) async fn has_analytics_events(pool: &PgPool, session_id: &SessionId) -> Result<bool> {
    let id = session_id.as_str();

    let result = sqlx::query_scalar!(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM analytics_events WHERE session_id = $1
        ) as "exists!"
        "#,
        id
    )
    .fetch_one(pool)
    .await?;

    Ok(result)
}

pub(crate) async fn count_unique_ips_by_fingerprint(
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

pub(crate) async fn count_engagement_events_by_fingerprint(
    pool: &PgPool,
    fingerprint_hash: &str,
    window_days: i64,
) -> Result<i64> {
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(e.id)::BIGINT as "count!"
        FROM engagement_events e
        JOIN user_sessions s ON s.session_id = e.session_id
        WHERE s.fingerprint_hash = $1
          AND s.started_at > CURRENT_TIMESTAMP - make_interval(days => $2)
        "#,
        fingerprint_hash,
        window_days as i32
    )
    .fetch_one(pool)
    .await?;

    Ok(count)
}

pub(crate) async fn get_session_starts_by_fingerprint(
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

pub(crate) async fn get_session_velocity(
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
