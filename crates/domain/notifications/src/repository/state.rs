//! Read, escalation and delivery state of notification rows.
//!
//! A read row that a digest still held is closed as `delivered`: there is
//! nothing left to announce. Escalation returns the row to `pending`, so
//! channels that deliver only `human` attention announce it; surfaces that
//! already hold a `delivered` receipt skip it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
                   held_until = NULL, delivery_state = $4
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

    pub async fn mark_delivered(&self, ids: &[NotificationId]) -> Result<u64, RepositoryError> {
        if ids.is_empty() {
            return Ok(0);
        }
        let ids: Vec<String> = ids.iter().map(|id| id.as_str().to_owned()).collect();
        let result = sqlx::query!(
            "UPDATE notifications SET delivery_state = $2
             WHERE id = ANY($1::text[]::uuid[]) AND delivery_state = $3",
            &ids,
            DeliveryState::Delivered.as_str(),
            DeliveryState::Pending.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }
}
