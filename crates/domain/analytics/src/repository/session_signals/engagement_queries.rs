//! Engagement counts for sessions resolved by their owner.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use sqlx::PgPool;
use systemprompt_identifiers::SessionId;

pub(super) async fn count_engagement_events_for_sessions(
    pool: &PgPool,
    session_ids: &[String],
) -> Result<i64> {
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(e.id)::BIGINT as "count!"
        FROM engagement_events e
        WHERE e.session_id = ANY($1)
        "#,
        session_ids,
    )
    .fetch_one(pool)
    .await?;

    Ok(count)
}

pub(super) async fn engaged_session_ids_since(
    pool: &PgPool,
    window_hours: i64,
) -> Result<Vec<SessionId>> {
    let ids = sqlx::query_scalar!(
        r#"
        SELECT DISTINCT e.session_id as "session_id!"
        FROM engagement_events e
        WHERE e.time_on_page_ms > 0
          AND e.created_at >= NOW() - make_interval(hours => $1::int)
        "#,
        i32::try_from(window_hours).unwrap_or(i32::MAX),
    )
    .fetch_all(pool)
    .await?;

    Ok(ids.into_iter().map(SessionId::new).collect())
}
