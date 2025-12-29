use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::{SessionId, UserId};

use crate::models::AnalyticsSession;

use super::types::{SessionBehavioralData, SessionRecord};

pub async fn find_by_id(pool: &DbPool, session_id: &SessionId) -> Result<Option<AnalyticsSession>> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let id = session_id.as_str();
    sqlx::query_as!(
        AnalyticsSession,
        r#"
        SELECT session_id, user_id, fingerprint_hash, ip_address, user_agent, device_type,
               browser, os, country, city, referrer_url, utm_source, utm_medium,
               utm_campaign, is_bot, is_scanner, is_behavioral_bot, behavioral_bot_reason,
               started_at, last_activity_at, ended_at, request_count, task_count,
               ai_request_count, message_count
        FROM user_sessions
        WHERE session_id = $1
        "#,
        id
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(Into::into)
}

pub async fn find_by_fingerprint(
    pool: &DbPool,
    fingerprint_hash: &str,
    user_id: &UserId,
) -> Result<Option<AnalyticsSession>> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let uid = user_id.as_str();
    sqlx::query_as!(
        AnalyticsSession,
        r#"
        SELECT session_id, user_id, fingerprint_hash, ip_address, user_agent, device_type,
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
    .fetch_optional(pool.as_ref())
    .await
    .map_err(Into::into)
}

pub async fn list_active_by_user(pool: &DbPool, user_id: &UserId) -> Result<Vec<AnalyticsSession>> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let uid = user_id.as_str();
    sqlx::query_as!(
        AnalyticsSession,
        r#"
        SELECT session_id, user_id, fingerprint_hash, ip_address, user_agent, device_type,
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
    .fetch_all(pool.as_ref())
    .await
    .map_err(Into::into)
}

pub async fn find_recent_by_fingerprint(
    pool: &DbPool,
    fingerprint_hash: &str,
    max_age_seconds: i64,
) -> Result<Option<SessionRecord>> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let cutoff = Utc::now() - Duration::seconds(max_age_seconds);
    sqlx::query_as!(
        SessionRecord,
        r#"
        SELECT session_id, user_id, expires_at
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
    .fetch_optional(pool.as_ref())
    .await
    .map_err(Into::into)
}

pub async fn exists(pool: &DbPool, session_id: &SessionId) -> Result<bool> {
    let pool = pool.pool_arc().context("Failed to get pool")?;
    let id = session_id.as_str();
    let result = sqlx::query_scalar!(
        r#"SELECT 1 as "exists" FROM user_sessions WHERE session_id = $1 LIMIT 1"#,
        id
    )
    .fetch_optional(&*pool)
    .await?;
    Ok(result.is_some())
}

pub async fn get_throttle_level(pool: &DbPool, session_id: &SessionId) -> Result<i32> {
    let pool = pool.pool_arc().context("Failed to get pool")?;
    let id = session_id.as_str();

    let result = sqlx::query_scalar!(
        r#"SELECT throttle_level FROM user_sessions WHERE session_id = $1"#,
        id
    )
    .fetch_optional(pool.as_ref())
    .await?;

    Ok(result.unwrap_or(0))
}

pub async fn count_sessions_by_fingerprint(
    pool: &DbPool,
    fingerprint_hash: &str,
    window_hours: i64,
) -> Result<i64> {
    let pool = pool.pool_arc().context("Failed to get pool")?;

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
    .fetch_one(pool.as_ref())
    .await?;

    Ok(count)
}

pub async fn get_endpoint_sequence(pool: &DbPool, session_id: &SessionId) -> Result<Vec<String>> {
    let pool = pool.pool_arc().context("Failed to get pool")?;
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
    .fetch_all(pool.as_ref())
    .await?;

    Ok(endpoints.into_iter().flatten().collect())
}

pub async fn get_request_timestamps(
    pool: &DbPool,
    session_id: &SessionId,
) -> Result<Vec<DateTime<Utc>>> {
    let pool = pool.pool_arc().context("Failed to get pool")?;
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
    .fetch_all(pool.as_ref())
    .await?;

    Ok(timestamps)
}

pub async fn get_total_content_pages(pool: &DbPool) -> Result<i64> {
    let pool = pool.pool_arc().context("Failed to get pool")?;

    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*)::BIGINT as "count!"
        FROM markdown_content
        WHERE public = true
        "#
    )
    .fetch_one(pool.as_ref())
    .await?;

    Ok(count)
}

pub async fn get_session_for_behavioral_analysis(
    pool: &DbPool,
    session_id: &SessionId,
) -> Result<Option<SessionBehavioralData>> {
    let pool = pool.pool_arc().context("Failed to get pool")?;
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
            last_activity_at as "last_activity_at!"
        FROM user_sessions
        WHERE session_id = $1
        "#,
        id
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(Into::into)
}
