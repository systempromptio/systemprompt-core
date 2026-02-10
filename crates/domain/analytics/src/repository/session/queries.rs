use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use systemprompt_identifiers::{SessionId, UserId};

use crate::models::AnalyticsSession;

use super::types::{SessionBehavioralData, SessionRecord};

pub async fn find_by_id(pool: &PgPool, session_id: &SessionId) -> Result<Option<AnalyticsSession>> {
    let id = session_id.as_str();
    sqlx::query_as!(
        AnalyticsSession,
        r#"
        SELECT session_id as "session_id: SessionId",
               user_id as "user_id?: UserId",
               fingerprint_hash, ip_address, user_agent, device_type,
               browser, os, country, city, referrer_url, utm_source, utm_medium,
               utm_campaign, is_bot, is_scanner, is_behavioral_bot, behavioral_bot_reason,
               started_at, last_activity_at, ended_at, request_count, task_count,
               ai_request_count, message_count
        FROM user_sessions
        WHERE session_id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

pub async fn find_by_fingerprint(
    pool: &PgPool,
    fingerprint_hash: &str,
    user_id: &UserId,
) -> Result<Option<AnalyticsSession>> {
    let uid = user_id.as_str();
    sqlx::query_as!(
        AnalyticsSession,
        r#"
        SELECT session_id as "session_id: SessionId",
               user_id as "user_id?: UserId",
               fingerprint_hash, ip_address, user_agent, device_type,
               browser, os, country, city, referrer_url, utm_source, utm_medium,
               utm_campaign, is_bot, is_scanner, is_behavioral_bot, behavioral_bot_reason,
               started_at, last_activity_at, ended_at, request_count, task_count,
               ai_request_count, message_count
        FROM user_sessions
        WHERE fingerprint_hash = $1 AND user_id = $2 AND ended_at IS NULL
        ORDER BY last_activity_at DESC
        LIMIT 1
        "#,
        fingerprint_hash,
        uid
    )
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

pub async fn list_active_by_user(pool: &PgPool, user_id: &UserId) -> Result<Vec<AnalyticsSession>> {
    let uid = user_id.as_str();
    sqlx::query_as!(
        AnalyticsSession,
        r#"
        SELECT session_id as "session_id: SessionId",
               user_id as "user_id?: UserId",
               fingerprint_hash, ip_address, user_agent, device_type,
               browser, os, country, city, referrer_url, utm_source, utm_medium,
               utm_campaign, is_bot, is_scanner, is_behavioral_bot, behavioral_bot_reason,
               started_at, last_activity_at, ended_at, request_count, task_count,
               ai_request_count, message_count
        FROM user_sessions
        WHERE user_id = $1 AND ended_at IS NULL
        ORDER BY last_activity_at DESC
        "#,
        uid
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn find_recent_by_fingerprint(
    pool: &PgPool,
    fingerprint_hash: &str,
    max_age_seconds: i64,
) -> Result<Option<SessionRecord>> {
    let cutoff = Utc::now() - Duration::seconds(max_age_seconds);
    sqlx::query_as!(
        SessionRecord,
        r#"
        SELECT
            session_id as "session_id: SessionId",
            user_id as "user_id: UserId",
            expires_at
        FROM user_sessions
        WHERE fingerprint_hash = $1
          AND last_activity_at > $2
          AND ended_at IS NULL
        ORDER BY last_activity_at DESC
        LIMIT 1
        "#,
        fingerprint_hash,
        cutoff
    )
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

pub async fn exists(pool: &PgPool, session_id: &SessionId) -> Result<bool> {
    let id = session_id.as_str();
    let result = sqlx::query_scalar!(
        r#"SELECT 1 as "exists" FROM user_sessions WHERE session_id = $1 LIMIT 1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(result.is_some())
}

pub async fn get_throttle_level(pool: &PgPool, session_id: &SessionId) -> Result<i32> {
    let id = session_id.as_str();

    let result = sqlx::query_scalar!(
        r#"SELECT throttle_level FROM user_sessions WHERE session_id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.unwrap_or(0))
}

pub async fn count_sessions_by_fingerprint(
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

pub async fn get_endpoint_sequence(pool: &PgPool, session_id: &SessionId) -> Result<Vec<String>> {
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

pub async fn get_request_timestamps(
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

pub async fn get_total_content_pages(pool: &PgPool) -> Result<i64> {
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*)::BIGINT as "count!"
        FROM markdown_content
        WHERE public = true
        "#
    )
    .fetch_one(pool)
    .await?;

    Ok(count)
}

pub async fn get_session_for_behavioral_analysis(
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

pub async fn has_analytics_events(pool: &PgPool, session_id: &SessionId) -> Result<bool> {
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

pub async fn get_session_velocity(
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
