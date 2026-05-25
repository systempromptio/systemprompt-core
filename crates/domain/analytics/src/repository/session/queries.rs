//! Read queries for `user_sessions` — finders, existence checks, throttle
//! lookups, and global session-volume counters. Behavioural-detector queries
//! live in [`super::behavioral_queries`].

use crate::Result;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use systemprompt_identifiers::{SessionId, UserId};

use crate::models::AnalyticsSession;

use super::types::{ActiveSessionLookup, SessionRecord};

pub(crate) async fn find_by_id(
    pool: &PgPool,
    session_id: &SessionId,
) -> Result<Option<AnalyticsSession>> {
    let id = session_id.as_str();
    sqlx::query_as!(
        AnalyticsSession,
        r#"
        SELECT session_id as "session_id: SessionId",
               user_id as "user_id?: UserId",
               fingerprint_hash, ip_address, user_agent, device_type,
               browser, os, country, city, referrer_url, utm_source, utm_medium,
               utm_campaign, utm_content, utm_term,
               is_bot, is_scanner, is_behavioral_bot, behavioral_bot_reason,
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

pub(crate) async fn find_by_fingerprint(
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
               utm_campaign, utm_content, utm_term,
               is_bot, is_scanner, is_behavioral_bot, behavioral_bot_reason,
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

pub(crate) async fn list_active_by_user(
    pool: &PgPool,
    user_id: &UserId,
) -> Result<Vec<AnalyticsSession>> {
    let uid = user_id.as_str();
    sqlx::query_as!(
        AnalyticsSession,
        r#"
        SELECT session_id as "session_id: SessionId",
               user_id as "user_id?: UserId",
               fingerprint_hash, ip_address, user_agent, device_type,
               browser, os, country, city, referrer_url, utm_source, utm_medium,
               utm_campaign, utm_content, utm_term,
               is_bot, is_scanner, is_behavioral_bot, behavioral_bot_reason,
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

pub(crate) async fn find_recent_by_fingerprint(
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

pub(crate) async fn find_active_by_id(
    pool: &PgPool,
    session_id: &SessionId,
) -> Result<Option<ActiveSessionLookup>> {
    let id = session_id.as_str();
    sqlx::query_as!(
        ActiveSessionLookup,
        r#"
        SELECT user_id as "user_id?: UserId"
        FROM user_sessions
        WHERE session_id = $1 AND revoked_at IS NULL
        "#,
        id
    )
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

pub(crate) async fn exists(pool: &PgPool, session_id: &SessionId) -> Result<bool> {
    let id = session_id.as_str();
    let result = sqlx::query_scalar!(
        r#"SELECT 1 as "exists" FROM user_sessions WHERE session_id = $1 LIMIT 1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(result.is_some())
}

pub(crate) async fn get_throttle_level(pool: &PgPool, session_id: &SessionId) -> Result<i32> {
    let id = session_id.as_str();

    let result = sqlx::query_scalar!(
        r#"SELECT throttle_level FROM user_sessions WHERE session_id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.unwrap_or(0))
}

pub(crate) async fn get_total_content_pages(pool: &PgPool) -> Result<i64> {
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

pub(crate) async fn count_inactive(pool: &PgPool, inactive_hours: i32) -> Result<i64> {
    let cutoff = Utc::now() - Duration::hours(i64::from(inactive_hours));
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*)::BIGINT as "count!"
        FROM user_sessions
        WHERE ended_at IS NULL AND last_activity_at < $1
        "#,
        cutoff,
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}
