//! Retention purges over notification rows, run by the retention job: read
//! rows, rows past their stream's retention, and unread rows past the
//! profile's unread age.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::NotificationStreamId;
use systemprompt_traits::RepositoryError;

use super::NotificationRepository;

impl NotificationRepository {
    pub async fn purge_read_before(
        &self,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query!(
            "DELETE FROM notifications WHERE id IN (
                 SELECT id FROM notifications
                 WHERE read_at IS NOT NULL AND read_at < $1 LIMIT $2)",
            cutoff,
            batch,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn purge_stream_retention(
        &self,
        stream: &NotificationStreamId,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query!(
            "DELETE FROM notifications WHERE id IN (
                 SELECT id FROM notifications
                 WHERE stream_id = $1 AND created_at < $2 LIMIT $3)",
            stream.as_str(),
            cutoff,
            batch,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn purge_unread_before(
        &self,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query!(
            "DELETE FROM notifications WHERE id IN (
                 SELECT id FROM notifications
                 WHERE read_at IS NULL AND created_at < $1 LIMIT $2)",
            cutoff,
            batch,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }
}
