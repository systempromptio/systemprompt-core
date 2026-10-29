//! [`NotificationRepository`]: per-recipient rows and the keyset feed.
//!
//! Coalescing folds a new event into the recipient's *unread* row with the
//! same key (`uq_notifications_unread_coalesce` is partial on
//! `read_at IS NULL`), so reading a row closes it to further folding.
//! Persistence, state changes, the delivery sweep, unread counts and
//! retention live in sibling modules on the same type.
//!
//! [`NotificationRepository::lock_pending`] row-locks the `pending` rows a
//! delivery pass announces with `SKIP LOCKED`, together with the
//! `delivery_seq` each was read at, so two passes never announce one row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::types::Json;
use sqlx::{PgConnection, PgExecutor};
use systemprompt_identifiers::{NotificationId, NotificationStreamId, UserId};
use systemprompt_models::notifications::{
    ActionPrompt, Attention, DeliveryState, Notification, SubjectRef,
};
use systemprompt_traits::RepositoryError;

use super::Pools;
use crate::models::{FeedFilter, NotificationRow};

#[derive(Debug, Clone)]
pub struct NotificationRepository {
    pub(super) pools: Pools,
}

impl NotificationRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn find(
        executor: impl PgExecutor<'_>,
        recipient: &UserId,
        id: &NotificationId,
    ) -> Result<Option<Notification>, RepositoryError> {
        Ok(
            Self::find_many(executor, recipient, std::slice::from_ref(id))
                .await?
                .into_iter()
                .next(),
        )
    }

    pub async fn find_pooled(
        &self,
        recipient: &UserId,
        id: &NotificationId,
    ) -> Result<Option<Notification>, RepositoryError> {
        Self::find(&*self.pools.read, recipient, id).await
    }

    pub async fn find_many(
        executor: impl PgExecutor<'_>,
        recipient: &UserId,
        ids: &[NotificationId],
    ) -> Result<Vec<Notification>, RepositoryError> {
        let ids: Vec<String> = ids.iter().map(|id| id.as_str().to_owned()).collect();
        sqlx::query_as!(
            NotificationRow,
            r#"SELECT n.id::text AS "id!: NotificationId",
                      n.recipient_user_id AS "recipient_user_id: UserId", n.kind,
                      n.stream_id AS "stream_id: NotificationStreamId",
                      n.actor_user_id AS "actor_user_id: UserId", u.display_name AS actor_display,
                      n.actor_is_agent, n.via_agent,
                      n.subjects AS "subjects: Json<Vec<SubjectRef>>", n.attention,
                      n.attention_reason, n.action AS "action: Json<ActionPrompt>", n.excerpt,
                      n.coalesce_count, n.first_event_at, n.last_event_at, n.created_at,
                      n.read_at, n.escalated_from, n.escalation_note, n.held_until,
                      n.delivery_state, n.delivery_seq
               FROM notifications n LEFT JOIN users u ON u.id = n.actor_user_id
               WHERE n.recipient_user_id = $1 AND n.id = ANY($2::text[]::uuid[])
               ORDER BY n.created_at DESC, n.id DESC"#,
            recipient.as_str(),
            &ids,
        )
        .fetch_all(executor)
        .await?
        .into_iter()
        .map(Notification::try_from)
        .collect()
    }

    pub async fn lock_pending(
        conn: &mut PgConnection,
        ids: &[NotificationId],
    ) -> Result<Vec<(Notification, i64)>, RepositoryError> {
        let ids: Vec<String> = ids.iter().map(|id| id.as_str().to_owned()).collect();
        sqlx::query_as!(
            NotificationRow,
            r#"SELECT n.id::text AS "id!: NotificationId",
                      n.recipient_user_id AS "recipient_user_id: UserId", n.kind,
                      n.stream_id AS "stream_id: NotificationStreamId",
                      n.actor_user_id AS "actor_user_id: UserId", u.display_name AS actor_display,
                      n.actor_is_agent, n.via_agent,
                      n.subjects AS "subjects: Json<Vec<SubjectRef>>", n.attention,
                      n.attention_reason, n.action AS "action: Json<ActionPrompt>", n.excerpt,
                      n.coalesce_count, n.first_event_at, n.last_event_at, n.created_at,
                      n.read_at, n.escalated_from, n.escalation_note, n.held_until,
                      n.delivery_state, n.delivery_seq
               FROM notifications n LEFT JOIN users u ON u.id = n.actor_user_id
               WHERE n.id = ANY($1::text[]::uuid[]) AND n.delivery_state = $2
               ORDER BY n.created_at, n.id
               FOR NO KEY UPDATE OF n SKIP LOCKED"#,
            &ids,
            DeliveryState::Pending.as_str(),
        )
        .fetch_all(conn)
        .await?
        .into_iter()
        .map(|row| {
            let seq = row.delivery_seq;
            Notification::try_from(row).map(|notification| (notification, seq))
        })
        .collect()
    }

    pub async fn list_feed(
        &self,
        recipient: &UserId,
        filter: FeedFilter,
    ) -> Result<Vec<Notification>, RepositoryError> {
        let (at, id) = filter
            .since
            .map(|cursor| (cursor.at, cursor.id.to_string()))
            .unzip();
        sqlx::query_as!(
            NotificationRow,
            r#"SELECT n.id::text AS "id!: NotificationId",
                      n.recipient_user_id AS "recipient_user_id: UserId", n.kind,
                      n.stream_id AS "stream_id: NotificationStreamId",
                      n.actor_user_id AS "actor_user_id: UserId", u.display_name AS actor_display,
                      n.actor_is_agent, n.via_agent,
                      n.subjects AS "subjects: Json<Vec<SubjectRef>>", n.attention,
                      n.attention_reason, n.action AS "action: Json<ActionPrompt>", n.excerpt,
                      n.coalesce_count, n.first_event_at, n.last_event_at, n.created_at,
                      n.read_at, n.escalated_from, n.escalation_note, n.held_until,
                      n.delivery_state, n.delivery_seq
               FROM notifications n LEFT JOIN users u ON u.id = n.actor_user_id
               WHERE n.recipient_user_id = $1
                 AND ($2::timestamptz IS NULL OR (n.created_at, n.id) < ($2, $3::text::uuid))
                 AND ($4::text IS NULL OR n.attention = $4)
                 AND (NOT $5 OR n.read_at IS NULL)
               ORDER BY n.created_at DESC, n.id DESC
               LIMIT $6"#,
            recipient.as_str(),
            at,
            id,
            filter.attention.map(Attention::as_str),
            filter.unread_only,
            filter.limit,
        )
        .fetch_all(&*self.pools.read)
        .await?
        .into_iter()
        .map(Notification::try_from)
        .collect()
    }
}
