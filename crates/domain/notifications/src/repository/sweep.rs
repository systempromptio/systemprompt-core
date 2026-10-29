//! The delivery sweep's queries: releasing digest-held rows whose window has
//! closed, finding `pending` rows past the grace period, and the backlog.
//!
//! The release claims rows under `FOR UPDATE SKIP LOCKED` and returns them
//! to `pending` in one bounded statement, so a crash between release and
//! delivery leaves them for the next sweep. The sweep itself runs under a
//! session advisory lock on a fixed key, so one replica sweeps at a time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::NotificationId;
use systemprompt_models::notifications::DeliveryState;
use systemprompt_traits::RepositoryError;

use super::NotificationRepository;

impl NotificationRepository {
    pub async fn release_held(
        &self,
        now: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<NotificationId>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"UPDATE notifications n SET delivery_state = $3, held_until = NULL
               FROM (SELECT id FROM notifications
                     WHERE delivery_state = $4 AND held_until <= $1
                     ORDER BY held_until LIMIT $2
                     FOR UPDATE SKIP LOCKED) due
               WHERE n.id = due.id
               RETURNING n.id::text AS "id!: NotificationId""#,
            now,
            limit,
            DeliveryState::Pending.as_str(),
            DeliveryState::Held.as_str(),
        )
        .fetch_all(&*self.pools.write)
        .await?)
    }

    pub async fn stale_pending(
        &self,
        before: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<NotificationId>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT id::text AS "id!: NotificationId" FROM notifications
               WHERE delivery_state = $3 AND created_at < $1
               ORDER BY created_at LIMIT $2"#,
            before,
            limit,
            DeliveryState::Pending.as_str(),
        )
        .fetch_all(&*self.pools.write)
        .await?)
    }

    pub async fn backlog(&self) -> Result<(i64, Option<i64>), RepositoryError> {
        let row = sqlx::query!(
            r#"SELECT COUNT(*) AS "pending!",
                      EXTRACT(EPOCH FROM now() - MIN(created_at))::BIGINT AS oldest
               FROM notifications WHERE delivery_state = $1"#,
            DeliveryState::Pending.as_str(),
        )
        .fetch_one(&*self.pools.read)
        .await?;
        Ok((row.pending, row.oldest))
    }
}

const SWEEP_LOCK: i64 = 0x6e6f_7469_6679;

impl NotificationRepository {
    pub async fn try_sweep_lock(conn: &mut sqlx::PgConnection) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT pg_try_advisory_lock($1) AS "locked!""#,
            SWEEP_LOCK
        )
        .fetch_one(conn)
        .await?)
    }

    pub async fn release_sweep_lock(conn: &mut sqlx::PgConnection) -> Result<(), RepositoryError> {
        sqlx::query_scalar!(
            r#"SELECT pg_advisory_unlock($1) AS "released!""#,
            SWEEP_LOCK
        )
        .fetch_one(conn)
        .await?;
        Ok(())
    }
}
