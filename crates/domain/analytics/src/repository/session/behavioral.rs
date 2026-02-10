use anyhow::Result;
use sqlx::PgPool;
use systemprompt_identifiers::SessionId;

pub async fn mark_as_behavioral_bot(
    pool: &PgPool,
    session_id: &SessionId,
    reason: &str,
) -> Result<()> {
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
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn check_and_mark_behavioral_bot(
    pool: &PgPool,
    session_id: &SessionId,
    request_count_threshold: i32,
) -> Result<bool> {
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
    .fetch_optional(pool)
    .await?;
    Ok(result.is_some())
}

pub async fn update_behavioral_detection(
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
