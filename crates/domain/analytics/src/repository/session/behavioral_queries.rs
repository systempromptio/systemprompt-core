//! Engagement counts for sessions resolved by their owner.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use sqlx::PgPool;

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
