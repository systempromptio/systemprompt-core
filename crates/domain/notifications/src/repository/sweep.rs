//! The delivery sweep's queries: releasing digest-held rows whose window has
//! closed, finding `pending` rows past the grace period, and the backlog.
//!
//! Every sweep query runs on the sweep's short selection transaction, which
//! first takes a transaction-scoped advisory lock on [`SWEEP_LOCK_KEY`], so
//! one replica selects at a time. The release returns due rows to `pending`
//! (advancing `delivery_seq` and `pending_since`); the stale selection keys
//! on `pending_since` and skips rows under a live delivery lease, so a row
//! whose deliverer stopped is selected again once its lease expires.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use systemprompt_identifiers::NotificationId;
use systemprompt_models::notifications::DeliveryState;
use systemprompt_traits::RepositoryError;

use super::NotificationRepository;

pub const SWEEP_LOCK_KEY: i64 = 0x6e6f_7469_6679;

impl NotificationRepository {
    pub async fn try_sweep_lock(conn: &mut PgConnection) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT pg_try_advisory_xact_lock($1) AS "locked!""#,
            SWEEP_LOCK_KEY
        )
        .fetch_one(conn)
        .await?)
    }

    pub async fn release_held(
        conn: &mut PgConnection,
        now: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<NotificationId>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"UPDATE notifications n
               SET delivery_state = $3, held_until = NULL,
                   delivery_seq = n.delivery_seq + 1, pending_since = now()
               FROM (SELECT id FROM notifications
                     WHERE delivery_state = $4 AND held_until <= $1
                     ORDER BY held_until LIMIT $2
                     FOR NO KEY UPDATE SKIP LOCKED) due
               WHERE n.id = due.id
               RETURNING n.id::text AS "id!: NotificationId""#,
            now,
            limit,
            DeliveryState::Pending.as_str(),
            DeliveryState::Held.as_str(),
        )
        .fetch_all(conn)
        .await?)
    }

    pub async fn stale_pending(
        conn: &mut PgConnection,
        before: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<NotificationId>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT id::text AS "id!: NotificationId" FROM notifications
               WHERE delivery_state = $3 AND pending_since < $1
                 AND (delivery_lease_until IS NULL OR delivery_lease_until <= now())
               ORDER BY pending_since LIMIT $2
               FOR NO KEY UPDATE SKIP LOCKED"#,
            before,
            limit,
            DeliveryState::Pending.as_str(),
        )
        .fetch_all(conn)
        .await?)
    }

    pub async fn backlog(&self) -> Result<(i64, Option<i64>), RepositoryError> {
        let row = sqlx::query!(
            r#"SELECT COUNT(*) AS "pending!",
                      EXTRACT(EPOCH FROM now() - MIN(pending_since))::BIGINT AS oldest
               FROM notifications WHERE delivery_state = $1"#,
            DeliveryState::Pending.as_str(),
        )
        .fetch_one(&*self.pools.read)
        .await?;
        Ok((row.pending, row.oldest))
    }
}
