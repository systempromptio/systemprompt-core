//! Fails `pending` audit rows whose settlement can no longer arrive: the
//! replica that held their receipt is gone, so their usage is unknown and is
//! recorded as such rather than left open forever.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::{AiRequestId, UserId};

use super::AiRequestRepository;
use crate::error::RepositoryError;

pub const ORPHANED_REASON: &str = "settlement never arrived; usage unknown";

/// How long a `pending` row may stay open before its settlement is treated as
/// lost. One hour is comfortably past the longest provider stream.
pub const ORPHAN_AGE: Duration = Duration::from_hours(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanedRequest {
    pub id: AiRequestId,
    pub owner: UserId,
}

impl AiRequestRepository {
    pub async fn fail_orphaned_pending(
        &self,
        older_than: Duration,
    ) -> Result<Vec<OrphanedRequest>, RepositoryError> {
        let age = i64::try_from(older_than.as_secs()).unwrap_or(i64::MAX);
        let rows = sqlx::query!(
            r#"
            UPDATE ai_requests
            SET status = 'failed', error_message = $2,
                completed_at = COALESCE(completed_at, CURRENT_TIMESTAMP),
                updated_at = CURRENT_TIMESTAMP
            WHERE status = 'pending'
              AND created_at < CURRENT_TIMESTAMP - make_interval(secs => $1::double precision)
            RETURNING id, user_id
            "#,
            age as f64,
            ORPHANED_REASON
        )
        .fetch_all(self.write_pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| OrphanedRequest {
                id: AiRequestId::new(row.id),
                owner: UserId::new(row.user_id),
            })
            .collect())
    }
}
