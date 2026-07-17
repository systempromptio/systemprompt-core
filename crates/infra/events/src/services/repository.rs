//! Persistence for the cross-replica `event_outbox`.
//!
//! `event_outbox` rows are the durable handoff between replicas: a routed
//! event is appended here and announced over Postgres `NOTIFY`; peer
//! replicas load the row and re-inject the event into their local
//! broadcasters. Every `event_outbox` statement lives in this repository —
//! [`EventRouter`](super::routing::EventRouter) and
//! [`PostgresEventBridge`](super::bridge::PostgresEventBridge) call it
//! rather than running SQL themselves.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use systemprompt_identifiers::{Actor, EventOutboxId, UserId};

use super::routing::{OUTBOX_CHANNEL, OutboxChannel};

pub(super) struct OutboxRow {
    pub channel: String,
    pub user_id: UserId,
    // JSON: the `payload` jsonb column is polymorphic by `channel`; the
    // relay decodes it into the matching typed event after dispatch.
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub(super) struct EventOutboxRepository {
    pool: PgPool,
}

impl EventOutboxRepository {
    pub(super) const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub(super) async fn insert(
        &self,
        id: &EventOutboxId,
        channel: OutboxChannel,
        actor: &Actor,
        payload: &serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        let (actor_kind, actor_id) = actor.audit_columns();
        sqlx::query!(
            "INSERT INTO event_outbox (id, channel, user_id, payload, actor_kind, actor_id) \
             VALUES ($1, $2, $3, $4, $5, $6)",
            id.as_str(),
            channel.as_str(),
            actor.user_id.as_str(),
            payload,
            actor_kind,
            actor_id,
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
    }

    pub(super) async fn notify(&self, id: &EventOutboxId) -> Result<(), sqlx::Error> {
        sqlx::query!("SELECT pg_notify($1, $2)", OUTBOX_CHANNEL, id.as_str())
            .execute(&self.pool)
            .await
            .map(|_| ())
    }

    pub(super) async fn find(&self, id: &EventOutboxId) -> Result<Option<OutboxRow>, sqlx::Error> {
        sqlx::query_as!(
            OutboxRow,
            r#"SELECT channel, user_id as "user_id!: UserId", payload FROM event_outbox WHERE id = $1"#,
            id.as_str(),
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub(super) async fn prune(
        &self,
        cutoff: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, sqlx::Error> {
        sqlx::query!("DELETE FROM event_outbox WHERE created_at < $1", cutoff)
            .execute(&self.pool)
            .await
            .map(|result| result.rows_affected())
    }
}
