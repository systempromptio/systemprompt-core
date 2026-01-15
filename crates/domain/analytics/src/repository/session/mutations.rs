use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::{SessionId, UserId};

use super::types::CreateSessionParams;

pub async fn update_activity(pool: &DbPool, session_id: &SessionId) -> Result<()> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let id = session_id.as_str();
    sqlx::query!(
        "UPDATE user_sessions SET last_activity_at = CURRENT_TIMESTAMP WHERE session_id = $1",
        id
    )
    .execute(pool.as_ref())
    .await?;
    Ok(())
}

pub async fn increment_request_count(pool: &DbPool, session_id: &SessionId) -> Result<()> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let id = session_id.as_str();
    sqlx::query!(
        "UPDATE user_sessions SET request_count = request_count + 1, last_activity_at = \
         CURRENT_TIMESTAMP WHERE session_id = $1",
        id
    )
    .execute(pool.as_ref())
    .await?;
    Ok(())
}

pub async fn increment_task_count(pool: &DbPool, session_id: &SessionId) -> Result<()> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let id = session_id.as_str();
    sqlx::query!(
        "UPDATE user_sessions SET task_count = task_count + 1, last_activity_at = \
         CURRENT_TIMESTAMP WHERE session_id = $1",
        id
    )
    .execute(pool.as_ref())
    .await?;
    Ok(())
}

pub async fn increment_ai_request_count(pool: &DbPool, session_id: &SessionId) -> Result<()> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let id = session_id.as_str();
    sqlx::query!(
        "UPDATE user_sessions SET ai_request_count = ai_request_count + 1, last_activity_at = \
         CURRENT_TIMESTAMP WHERE session_id = $1",
        id
    )
    .execute(pool.as_ref())
    .await?;
    Ok(())
}

pub async fn increment_message_count(pool: &DbPool, session_id: &SessionId) -> Result<()> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let id = session_id.as_str();
    sqlx::query!(
        "UPDATE user_sessions SET message_count = message_count + 1, last_activity_at = \
         CURRENT_TIMESTAMP WHERE session_id = $1",
        id
    )
    .execute(pool.as_ref())
    .await?;
    Ok(())
}

pub async fn end_session(pool: &DbPool, session_id: &SessionId) -> Result<()> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let id = session_id.as_str();
    sqlx::query!(
        "UPDATE user_sessions SET ended_at = CURRENT_TIMESTAMP WHERE session_id = $1",
        id
    )
    .execute(pool.as_ref())
    .await?;
    Ok(())
}

pub async fn mark_as_scanner(pool: &DbPool, session_id: &SessionId) -> Result<()> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let id = session_id.as_str();
    sqlx::query!(
        "UPDATE user_sessions SET is_scanner = true WHERE session_id = $1",
        id
    )
    .execute(pool.as_ref())
    .await?;
    Ok(())
}

pub async fn mark_as_behavioral_bot(
    pool: &DbPool,
    session_id: &SessionId,
    reason: &str,
) -> Result<()> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let id = session_id.as_str();
    sqlx::query!(
        r#"
        UPDATE user_sessions
        SET is_behavioral_bot = true,
            behavioral_bot_reason = $1
        WHERE session_id = $2
        "#,
        reason,
        id
    )
    .execute(pool.as_ref())
    .await?;
    Ok(())
}

pub async fn check_and_mark_behavioral_bot(
    pool: &DbPool,
    session_id: &SessionId,
    request_count_threshold: i32,
) -> Result<bool> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let id = session_id.as_str();
    let result = sqlx::query!(
        r#"
        UPDATE user_sessions
        SET is_behavioral_bot = true,
            behavioral_bot_reason = 'request_count_exceeded'
        WHERE session_id = $1
          AND request_count > $2
          AND is_bot = false
          AND is_scanner = false
          AND is_behavioral_bot = false
        RETURNING session_id
        "#,
        id,
        request_count_threshold
    )
    .fetch_optional(pool.as_ref())
    .await?;
    Ok(result.is_some())
}

pub async fn cleanup_inactive(pool: &DbPool, inactive_hours: i32) -> Result<u64> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let cutoff = Utc::now() - Duration::hours(i64::from(inactive_hours));
    let result = sqlx::query!(
        r#"
        UPDATE user_sessions
        SET ended_at = CURRENT_TIMESTAMP
        WHERE ended_at IS NULL AND last_activity_at < $1
        "#,
        cutoff
    )
    .execute(pool.as_ref())
    .await?;
    Ok(result.rows_affected())
}

pub async fn migrate_user_sessions(
    pool: &DbPool,
    old_user_id: &UserId,
    new_user_id: &UserId,
) -> Result<u64> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let old_id = old_user_id.as_str();
    let new_id = new_user_id.as_str();
    let result = sqlx::query!(
        "UPDATE user_sessions SET user_id = $1 WHERE user_id = $2",
        new_id,
        old_id
    )
    .execute(pool.as_ref())
    .await?;
    Ok(result.rows_affected())
}

#[allow(clippy::cognitive_complexity)]
pub async fn create_session(pool: &DbPool, params: &CreateSessionParams<'_>) -> Result<()> {
    let pool = pool.pool_arc().context("Failed to get database pool")?;
    let session_id = params.session_id.as_str();
    let user_id = params.user_id.map(UserId::as_str);
    let session_source = params.session_source.as_str();
    sqlx::query!(
        r#"
        INSERT INTO user_sessions (
            session_id, user_id, session_source, fingerprint_hash, ip_address, user_agent,
            device_type, browser, os, country, region, city, preferred_locale,
            referrer_source, referrer_url, landing_page, entry_url,
            utm_source, utm_medium, utm_campaign, is_bot, expires_at,
            started_at, last_activity_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
        session_id,
        user_id,
        session_source,
        params.fingerprint_hash,
        params.ip_address,
        params.user_agent,
        params.device_type,
        params.browser,
        params.os,
        params.country,
        params.region,
        params.city,
        params.preferred_locale,
        params.referrer_source,
        params.referrer_url,
        params.landing_page,
        params.entry_url,
        params.utm_source,
        params.utm_medium,
        params.utm_campaign,
        params.is_bot,
        params.expires_at
    )
    .execute(pool.as_ref())
    .await?;
    Ok(())
}

pub async fn increment_ai_usage(
    pool: &DbPool,
    session_id: &SessionId,
    tokens: i32,
    cost_cents: i32,
) -> Result<()> {
    let pool = pool.pool_arc().context("Failed to get pool")?;
    let id = session_id.as_str();
    let cost_cents_i64 = i64::from(cost_cents);
    sqlx::query!(
        r#"
        UPDATE user_sessions
        SET ai_request_count = COALESCE(ai_request_count, 0) + 1,
            total_tokens_used = COALESCE(total_tokens_used, 0) + $1,
            total_ai_cost_cents = COALESCE(total_ai_cost_cents, 0) + $2,
            last_activity_at = CURRENT_TIMESTAMP
        WHERE session_id = $3
        "#,
        tokens,
        cost_cents_i64,
        id
    )
    .execute(&*pool)
    .await?;
    Ok(())
}

pub async fn update_behavioral_detection(
    pool: &DbPool,
    session_id: &SessionId,
    score: i32,
    is_behavioral_bot: bool,
    reason: Option<&str>,
) -> Result<()> {
    let pool = pool.pool_arc().context("Failed to get pool")?;
    let id = session_id.as_str();

    sqlx::query!(
        r#"
        UPDATE user_sessions
        SET behavioral_bot_score = $1,
            is_behavioral_bot = $2,
            behavioral_bot_reason = $3,
            last_activity_at = CURRENT_TIMESTAMP
        WHERE session_id = $4
        "#,
        score,
        is_behavioral_bot,
        reason,
        id
    )
    .execute(pool.as_ref())
    .await?;

    Ok(())
}

pub async fn escalate_throttle(
    pool: &DbPool,
    session_id: &SessionId,
    new_level: i32,
) -> Result<()> {
    let pool = pool.pool_arc().context("Failed to get pool")?;
    let id = session_id.as_str();

    sqlx::query!(
        r#"
        UPDATE user_sessions
        SET throttle_level = $1,
            throttle_escalated_at = CURRENT_TIMESTAMP
        WHERE session_id = $2
        "#,
        new_level,
        id
    )
    .execute(pool.as_ref())
    .await?;

    Ok(())
}
