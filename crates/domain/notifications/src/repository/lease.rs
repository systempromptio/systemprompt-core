//! [`DeliveryLease`], the proof of a delivery claim on one notification row,
//! and [`Fenced`], the outcome of a write made under it.
//!
//! [`NotificationRepository::claim_pending`] is the only constructor: one
//! auto-committed statement sets a fresh token and an expiry on every
//! `pending` row whose lease is absent or expired, skipping rows another
//! statement holds. Channels then run with no connection held.
//! [`NotificationRepository::finish_claim`] clears the lease, and closes the
//! row as `delivered` when asked to and its `delivery_seq` is still the one
//! claimed, only while the row still carries the claim's token; a claim the
//! sweep took over after expiry gets [`Fenced::LeaseLost`] and its caller
//! writes nothing else for the row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use sqlx::PgConnection;
use systemprompt_identifiers::NotificationId;
use systemprompt_models::notifications::DeliveryState;
use systemprompt_traits::RepositoryError;

use super::NotificationRepository;

/// Ownership of one delivery claim of one notification row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryLease {
    id: NotificationId,
    token: String,
    seq: i64,
}

impl DeliveryLease {
    #[must_use]
    pub const fn id(&self) -> &NotificationId {
        &self.id
    }

    #[must_use]
    pub const fn seq(&self) -> i64 {
        self.seq
    }
}

/// Whether a write under a [`DeliveryLease`] took effect.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fenced {
    Applied,
    LeaseLost,
}

impl NotificationRepository {
    pub async fn claim_pending(
        &self,
        ids: &[NotificationId],
        lease_for: Duration,
    ) -> Result<Vec<DeliveryLease>, RepositoryError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<String> = ids.iter().map(|id| id.as_str().to_owned()).collect();
        let secs = lease_for.as_secs_f64();
        let rows = sqlx::query!(
            r#"UPDATE notifications n
               SET delivery_lease_token = gen_random_uuid(),
                   delivery_lease_until = now() + make_interval(secs => $3)
               FROM (SELECT id FROM notifications
                     WHERE id = ANY($1::text[]::uuid[]) AND delivery_state = $2
                       AND (delivery_lease_until IS NULL OR delivery_lease_until <= now())
                     FOR NO KEY UPDATE SKIP LOCKED) c
               WHERE n.id = c.id
               RETURNING n.id::text AS "id!: NotificationId",
                         n.delivery_lease_token::text AS "token!", n.delivery_seq"#,
            &ids,
            DeliveryState::Pending.as_str(),
            secs,
        )
        .fetch_all(&*self.pools.write)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| DeliveryLease {
                id: row.id,
                token: row.token,
                seq: row.delivery_seq,
            })
            .collect())
    }

    pub async fn finish_claim(
        conn: &mut PgConnection,
        lease: &DeliveryLease,
        close: bool,
    ) -> Result<Fenced, RepositoryError> {
        let result = sqlx::query!(
            "UPDATE notifications
             SET delivery_lease_token = NULL, delivery_lease_until = NULL,
                 delivery_state = CASE WHEN $3 AND delivery_seq = $4 THEN $5
                                       ELSE delivery_state END
             WHERE id = $1::text::uuid AND delivery_lease_token = $2::text::uuid
               AND delivery_state = $6",
            lease.id.as_str(),
            lease.token,
            close,
            lease.seq,
            DeliveryState::Delivered.as_str(),
            DeliveryState::Pending.as_str(),
        )
        .execute(conn)
        .await?;
        Ok(if result.rows_affected() == 0 {
            Fenced::LeaseLost
        } else {
            Fenced::Applied
        })
    }
}
