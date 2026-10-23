//! Behavioural-bot persistence for sessions: the only module that sets
//! `user_sessions.is_behavioral_bot`.
//!
//! Every statement here writes [`BehavioralBotReason::as_str`] and
//! [`BehavioralBotReason::score`] together, and selects its candidates
//! through `is_human_traffic()` so a session already classified by any other
//! signal is never re-judged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use sqlx::PgPool;
use systemprompt_identifiers::{SessionId, SessionSource};
use systemprompt_models::analytics::BehavioralBotReason;

pub(super) async fn mark_as_behavioral_bot(
    pool: &PgPool,
    session_id: &SessionId,
    reason: BehavioralBotReason,
) -> Result<()> {
    let id = session_id.as_str();
    sqlx::query!(
        r#"
        UPDATE user_sessions
        SET is_behavioral_bot = true,
            behavioral_bot_reason = $1,
            behavioral_bot_score = GREATEST(behavioral_bot_score, $2)
        WHERE session_id = $3
        "#,
        reason.as_str(),
        reason.score(),
        id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn check_and_mark_behavioral_bot(
    pool: &PgPool,
    session_id: &SessionId,
    request_count_threshold: i32,
) -> Result<bool> {
    let id = session_id.as_str();
    let reason = BehavioralBotReason::RequestCountExceeded;
    let result = sqlx::query!(
        r#"
        UPDATE user_sessions
        SET is_behavioral_bot = true,
            behavioral_bot_reason = $1,
            behavioral_bot_score = GREATEST(behavioral_bot_score, $2)
        WHERE session_id = $3
          AND request_count > $4
          AND is_human_traffic(is_bot, is_ai_crawler, is_scanner, is_behavioral_bot)
        RETURNING session_id
        "#,
        reason.as_str(),
        reason.score(),
        id,
        request_count_threshold
    )
    .fetch_optional(pool)
    .await?;
    Ok(result.is_some())
}

pub(super) async fn update_behavioral_detection(
    pool: &PgPool,
    session_id: &SessionId,
    score: i32,
    is_behavioral_bot: bool,
    reason: Option<&str>,
) -> Result<()> {
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
    .execute(pool)
    .await?;

    Ok(())
}

// Why: both heuristics are browser heuristics judged in a session's first day.
// Bridge, OAuth and API sessions never load a landing page or run JavaScript,
// so only `web` sessions are candidates; the lower bound stops every
// unflagged session ever recorded being re-judged on each run.
pub(super) async fn mark_ghost_sessions_as_bots(pool: &PgPool) -> Result<u64> {
    let reason = BehavioralBotReason::GhostSession;
    let marked = sqlx::query_scalar!(
        r#"
        WITH cleaned AS (
            UPDATE user_sessions
            SET is_behavioral_bot = true,
                behavioral_bot_reason = $1,
                behavioral_bot_score = GREATEST(behavioral_bot_score, $2)
            WHERE is_human_traffic(is_bot, is_ai_crawler, is_scanner, is_behavioral_bot)
              AND session_source = $3
              AND request_count = 0
              AND landing_page IS NULL
              AND entry_url IS NULL
              AND started_at < NOW() - INTERVAL '5 minutes'
              AND started_at >= NOW() - INTERVAL '1 day'
            RETURNING 1
        )
        SELECT COUNT(*)::BIGINT as "count!" FROM cleaned
        "#,
        reason.as_str(),
        reason.score(),
        SessionSource::Web.as_str()
    )
    .fetch_one(pool)
    .await?;
    Ok(u64::try_from(marked).unwrap_or(0))
}

// Why: engagement rows belong to the analytics domain, so the caller passes
// the sessions that did report time on page and this statement excludes them.
pub(super) async fn mark_no_js_sessions_as_bots(
    pool: &PgPool,
    engaged_session_ids: &[String],
) -> Result<u64> {
    let reason = BehavioralBotReason::NoJavascript;
    let marked = sqlx::query_scalar!(
        r#"
        WITH cleaned AS (
            UPDATE user_sessions
            SET is_behavioral_bot = true,
                behavioral_bot_reason = $1,
                behavioral_bot_score = GREATEST(behavioral_bot_score, $2)
            WHERE is_human_traffic(is_bot, is_ai_crawler, is_scanner, is_behavioral_bot)
              AND session_source = $3
              AND request_count > 0
              AND started_at < NOW() - INTERVAL '10 minutes'
              AND started_at >= NOW() - INTERVAL '1 day'
              AND NOT (session_id = ANY($4))
            RETURNING 1
        )
        SELECT COUNT(*)::BIGINT as "count!" FROM cleaned
        "#,
        reason.as_str(),
        reason.score(),
        SessionSource::Web.as_str(),
        engaged_session_ids
    )
    .fetch_one(pool)
    .await?;
    Ok(u64::try_from(marked).unwrap_or(0))
}
