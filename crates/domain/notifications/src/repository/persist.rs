//! Persisting one fact's recipients: one `INSERT … SELECT FROM UNNEST` for
//! the rows, folding into each recipient's unread row with the same key, and
//! one for the fact ledger that makes a re-processed fact idempotent.
//!
//! A fold replaces the excerpt, subjects and action and advances
//! `last_event_at` and `coalesce_count` together; the attention only rises.
//! A row is `held` while a digest holds it, else `pending`; a fold onto a
//! `delivered` row stays `delivered` unless it raises the attention. A fold
//! that leaves the row `pending` advances `delivery_seq` and restarts
//! `pending_since`, so a delivery pass that read the row before the fold
//! cannot mark it `delivered`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use sqlx::PgConnection;
use systemprompt_identifiers::{EventOutboxId, NotificationId, NotificationStreamId, UserId};
use systemprompt_traits::RepositoryError;

use super::NotificationRepository;
use crate::models::{NewNotifications, PersistedRow, parse};

impl NotificationRepository {
    pub async fn recorded_recipients(
        conn: &mut PgConnection,
        outbox_id: &EventOutboxId,
        recipients: &[UserId],
    ) -> Result<HashSet<UserId>, RepositoryError> {
        let recipients: Vec<String> = recipients.iter().map(|u| u.as_str().to_owned()).collect();
        Ok(sqlx::query_scalar!(
            r#"SELECT recipient_user_id AS "recipient!: UserId" FROM notification_facts
               WHERE outbox_id = $1 AND recipient_user_id = ANY($2::text[])"#,
            outbox_id.as_str(),
            &recipients,
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .collect())
    }

    pub async fn persist(
        conn: &mut PgConnection,
        batch: &NewNotifications,
    ) -> Result<Vec<PersistedRow>, RepositoryError> {
        if batch.recipients.is_empty() {
            return Ok(Vec::new());
        }
        let persisted = Self::upsert(conn, batch).await?;
        Self::record_facts(conn, &batch.outbox_id, &persisted).await?;
        Ok(persisted)
    }

    async fn upsert(
        conn: &mut PgConnection,
        batch: &NewNotifications,
    ) -> Result<Vec<PersistedRow>, RepositoryError> {
        let cols = RecipientColumns::new(batch);
        let rows = sqlx::query!(
            r#"INSERT INTO notifications
                   (recipient_user_id, kind, stream_id, actor_user_id, actor_is_agent, via_agent, subjects,
                    primary_subject_kind, primary_subject_id, attention, attention_reason, action, excerpt,
                    coalesce_key, first_event_at, last_event_at, outbox_id, held_until, delivery_state)
               SELECT r.recipient, $1, $2, $3, $4, $5, $6, $7, $8, r.attention, r.reason, $9,
                      r.excerpt, r.coalesce_key, $10, $10, $11, r.held_until,
                      CASE WHEN r.held_until IS NULL THEN 'pending' ELSE 'held' END
               FROM UNNEST($12::text[], $13::text[], $14::text[], $15::text[], $16::text[], $17::timestamptz[])
                    AS r(recipient, attention, reason, excerpt, coalesce_key, held_until)
               ON CONFLICT (recipient_user_id, coalesce_key) WHERE read_at IS NULL
               DO UPDATE SET
                   coalesce_count = notifications.coalesce_count + 1,
                   last_event_at = EXCLUDED.last_event_at,
                   excerpt = EXCLUDED.excerpt,
                   subjects = EXCLUDED.subjects,
                   action = EXCLUDED.action,
                   attention = CASE WHEN array_position(ARRAY['silent','agent','human'], EXCLUDED.attention) > array_position(ARRAY['silent','agent','human'], notifications.attention)
                       THEN EXCLUDED.attention ELSE notifications.attention END,
                   attention_reason = CASE WHEN array_position(ARRAY['silent','agent','human'], EXCLUDED.attention) > array_position(ARRAY['silent','agent','human'], notifications.attention)
                       THEN EXCLUDED.attention_reason ELSE notifications.attention_reason END,
                   held_until = CASE WHEN notifications.delivery_state = 'delivered'
                       AND array_position(ARRAY['silent','agent','human'], EXCLUDED.attention)
                       <= array_position(ARRAY['silent','agent','human'], notifications.attention)
                       THEN NULL ELSE COALESCE(notifications.held_until, EXCLUDED.held_until) END,
                   delivery_state = CASE WHEN notifications.delivery_state = 'delivered'
                       AND array_position(ARRAY['silent','agent','human'], EXCLUDED.attention)
                       <= array_position(ARRAY['silent','agent','human'], notifications.attention) THEN 'delivered'
                       WHEN COALESCE(notifications.held_until, EXCLUDED.held_until) IS NULL
                       THEN 'pending' ELSE 'held' END,
                   delivery_seq = CASE WHEN NOT (notifications.delivery_state = 'delivered'
                       AND array_position(ARRAY['silent','agent','human'], EXCLUDED.attention)
                       <= array_position(ARRAY['silent','agent','human'], notifications.attention))
                       AND COALESCE(notifications.held_until, EXCLUDED.held_until) IS NULL THEN
                       notifications.delivery_seq + 1 ELSE notifications.delivery_seq END,
                   pending_since = CASE WHEN NOT (notifications.delivery_state = 'delivered'
                       AND array_position(ARRAY['silent','agent','human'], EXCLUDED.attention)
                       <= array_position(ARRAY['silent','agent','human'], notifications.attention))
                       AND COALESCE(notifications.held_until, EXCLUDED.held_until) IS NULL THEN now()
                       ELSE notifications.pending_since END
               RETURNING id::text AS "id!: NotificationId", recipient_user_id AS "recipient!: UserId",
                         (xmax = 0) AS "inserted!", delivery_state"#,
            batch.kind.as_str(),
            batch.stream.as_ref().map(NotificationStreamId::as_str),
            batch.actor.as_str(),
            batch.actor_is_agent,
            batch.via_agent,
            serde_json::to_value(&batch.subjects)?,
            batch.subjects.first().map(|s| s.kind.as_str()),
            batch.subjects.first().map(|s| s.id.as_str()),
            serde_json::to_value(&batch.action)?,
            batch.event_at,
            batch.outbox_id.as_str(),
            &cols.users,
            &cols.attentions,
            &cols.reasons,
            &cols.excerpts as _,
            &cols.keys,
            &cols.held as _,
        )
        .fetch_all(&mut *conn)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(PersistedRow {
                    id: row.id,
                    recipient: row.recipient,
                    inserted: row.inserted,
                    delivery_state: parse("notifications.delivery_state", &row.delivery_state)?,
                })
            })
            .collect()
    }
    async fn record_facts(
        conn: &mut PgConnection,
        outbox_id: &EventOutboxId,
        persisted: &[PersistedRow],
    ) -> Result<(), RepositoryError> {
        let ids: Vec<String> = persisted.iter().map(|p| p.id.as_str().to_owned()).collect();
        let owners: Vec<String> = persisted
            .iter()
            .map(|p| p.recipient.as_str().to_owned())
            .collect();
        sqlx::query!(
            "INSERT INTO notification_facts (notification_id, outbox_id, recipient_user_id)
             SELECT f.id::uuid, $1, f.recipient FROM UNNEST($2::text[], $3::text[]) AS f(id, recipient)
             ON CONFLICT DO NOTHING",
            outbox_id.as_str(),
            &ids,
            &owners,
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }
}

struct RecipientColumns {
    users: Vec<String>,
    attentions: Vec<String>,
    reasons: Vec<String>,
    excerpts: Vec<Option<String>>,
    keys: Vec<String>,
    held: Vec<Option<chrono::DateTime<chrono::Utc>>>,
}

impl RecipientColumns {
    fn new(batch: &NewNotifications) -> Self {
        let r = &batch.recipients;
        Self {
            users: r.iter().map(|r| r.user.as_str().to_owned()).collect(),
            attentions: r.iter().map(|r| r.attention.as_str().to_owned()).collect(),
            reasons: r.iter().map(|r| r.reason.to_string()).collect(),
            excerpts: r.iter().map(|r| r.excerpt.clone()).collect(),
            keys: r.iter().map(|r| r.coalesce_key.clone()).collect(),
            held: r.iter().map(|r| r.held_until).collect(),
        }
    }
}
