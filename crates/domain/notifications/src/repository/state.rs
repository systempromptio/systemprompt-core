//! Read, escalation and delivery state of notification rows.
//!
//! A read row that a digest still held is closed as `delivered`: there is
//! nothing left to announce. Escalation returns the row to `pending`, so
//! channels that deliver only `human` attention announce it; surfaces that
//! already hold a `delivered` receipt skip it.
//!
//! Every transition to `pending` advances `delivery_seq` and restarts
//! `pending_since`. [`NotificationRepository::mark_delivered`] closes a row
//! only at the sequence its delivery pass read, so a row re-pended while the
//! pass ran stays `pending` for the pass that announces the change.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::{NotificationId, UserId};
use systemprompt_models::notifications::{DeliveryState, Notification};
use systemprompt_traits::RepositoryError;

use super::NotificationRepository;

impl NotificationRepository {
    pub async fn mark_read(
        &self,
        recipient: &UserId,
        ids: &[NotificationId],
    ) -> Result<Vec<NotificationId>, RepositoryError> {
        let ids: Vec<String> = ids.iter().map(|id| id.as_str().to_owned()).collect();
        Ok(sqlx::query_scalar!(
            r#"UPDATE notifications SET read_at = now(), held_until = NULL,
                   delivery_state = CASE WHEN delivery_state = 'held' THEN 'delivered'
                                         ELSE delivery_state END
               WHERE recipient_user_id = $1 AND id = ANY($2::text[]::uuid[]) AND read_at IS NULL
               RETURNING id::text AS "id!: NotificationId""#,
            recipient.as_str(),
            &ids,
        )
        .fetch_all(&*self.pools.write)
        .await?)
    }

    pub async fn mark_all_read(
        &self,
        recipient: &UserId,
    ) -> Result<Vec<NotificationId>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"UPDATE notifications SET read_at = now(), held_until = NULL,
                   delivery_state = CASE WHEN delivery_state = 'held' THEN 'delivered'
                                         ELSE delivery_state END
               WHERE recipient_user_id = $1 AND read_at IS NULL
               RETURNING id::text AS "id!: NotificationId""#,
            recipient.as_str(),
        )
        .fetch_all(&*self.pools.write)
        .await?)
    }

    pub async fn escalate(
        &self,
        recipient: &UserId,
        id: &NotificationId,
        note: Option<&str>,
    ) -> Result<Option<Notification>, RepositoryError> {
        let mut tx = self.pools.write.begin().await?;
        let updated = sqlx::query_scalar!(
            r#"UPDATE notifications
               SET escalated_from = attention, attention = 'human',
                   attention_reason = 'escalated', escalation_note = $3,
                   held_until = NULL, delivery_state = $4,
                   delivery_seq = delivery_seq + 1, pending_since = now()
               WHERE recipient_user_id = $1 AND id = $2::text::uuid AND attention = 'agent'
               RETURNING id::text AS "id!: NotificationId""#,
            recipient.as_str(),
            id.as_str(),
            note,
            DeliveryState::Pending.as_str(),
        )
        .fetch_optional(&mut *tx)
        .await?;
        let notification = match updated {
            Some(id) => Self::find(&mut *tx, recipient, &id).await?,
            None => None,
        };
        tx.commit().await?;
        Ok(notification)
    }

    pub async fn mark_delivered(
        conn: &mut PgConnection,
        rows: &[(NotificationId, i64)],
    ) -> Result<u64, RepositoryError> {
        if rows.is_empty() {
            return Ok(0);
        }
        let ids: Vec<String> = rows.iter().map(|(id, _)| id.as_str().to_owned()).collect();
        let seqs: Vec<i64> = rows.iter().map(|(_, seq)| *seq).collect();
        let result = sqlx::query!(
            "UPDATE notifications n SET delivery_state = $3
             FROM UNNEST($1::text[], $2::bigint[]) AS d(id, seq)
             WHERE n.id = d.id::uuid AND n.delivery_seq = d.seq AND n.delivery_state = $4",
            &ids,
            &seqs,
            DeliveryState::Delivered.as_str(),
            DeliveryState::Pending.as_str(),
        )
        .execute(conn)
        .await?;
        Ok(result.rows_affected())
    }
}
