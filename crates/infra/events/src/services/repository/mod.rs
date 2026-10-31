//! Persistence for the cross-replica `event_outbox`.
//!
//! `event_outbox` rows are the durable handoff between replicas: a routed
//! event is appended here and announced over Postgres `NOTIFY`; peer
//! replicas load the row and re-inject the event into their local
//! broadcasters. [`EventRouter`](super::routing::EventRouter) and
//! [`PostgresEventBridge`](super::bridge::PostgresEventBridge) use this
//! repository; the transactional outbox queries behind
//! [`DurableOutbox`](super::durable::DurableOutbox) live in [`durable`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) mod durable;
mod ownership;

use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use systemprompt_identifiers::{Actor, EventOutboxId, InstanceId, UserId};

use super::routing::{OUTBOX_CHANNEL, OutboxChannel};

pub use ownership::EventsOwnerReassignment;

// Why: the bridge prunes on every relay tick, so one bounded batch per tick
// keeps the statement short and leaves the remainder for the next tick.
const PRUNE_BATCH_ROWS: i64 = 5000;

pub(super) struct OutboxRow {
    pub id: EventOutboxId,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub channel: String,
    pub user_id: UserId,
    pub origin_instance_id: InstanceId,
    pub deliver_to_origin: bool,
    // JSON: the `payload` jsonb column is polymorphic by `channel`; the
    // relay decodes it into the matching typed event after dispatch.
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct OutboxInsert<'a> {
    pub id: &'a EventOutboxId,
    pub channel: OutboxChannel,
    pub actor: &'a Actor,
    // JSON: the event body, opaque here, stored as the outbox's JSONB `payload`.
    pub payload: &'a serde_json::Value,
}

#[derive(Debug, Clone)]
pub(super) struct EventOutboxRepository {
    pool: PgPool,
    instance_id: InstanceId,
}

impl EventOutboxRepository {
    pub(super) const fn new(pool: PgPool, instance_id: InstanceId) -> Self {
        Self { pool, instance_id }
    }

    pub(super) const fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    pub(super) async fn begin(&self) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
        self.pool.begin().await
    }

    pub(super) async fn insert(
        &self,
        conn: &mut PgConnection,
        row: OutboxInsert<'_>,
    ) -> Result<(), sqlx::Error> {
        let OutboxInsert {
            id,
            channel,
            actor,
            payload,
        } = row;
        let (actor_kind, actor_id) = actor.audit_columns();
        sqlx::query!(
            "INSERT INTO event_outbox (id, channel, user_id, payload, actor_kind, actor_id, \
             origin_instance_id) VALUES ($1, $2, $3, $4, $5, $6, $7)",
            id.as_str(),
            channel.as_str(),
            actor.user_id.as_str(),
            payload,
            actor_kind,
            actor_id,
            self.instance_id.as_str(),
        )
        .execute(&mut *conn)
        .await
        .map(|_| ())
    }

    pub(super) async fn notify(
        conn: &mut PgConnection,
        id: &EventOutboxId,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!("SELECT pg_notify($1, $2)", OUTBOX_CHANNEL, id.as_str())
            .execute(&mut *conn)
            .await
            .map(|_| ())
    }

    pub(super) async fn find(&self, id: &EventOutboxId) -> Result<Option<OutboxRow>, sqlx::Error> {
        sqlx::query_as!(
            OutboxRow,
            r#"
            SELECT
                id as "id: EventOutboxId",
                created_at,
                channel,
                user_id as "user_id: UserId",
                payload,
                origin_instance_id as "origin_instance_id: InstanceId",
                deliver_to_origin
            FROM event_outbox
            WHERE id = $1
            "#,
            id.as_str(),
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub(super) async fn list_since(
        &self,
        since: chrono::DateTime<chrono::Utc>,
        limit: i64,
    ) -> Result<Vec<OutboxRow>, sqlx::Error> {
        sqlx::query_as!(
            OutboxRow,
            r#"
            SELECT
                id as "id: EventOutboxId",
                created_at,
                channel,
                user_id as "user_id: UserId",
                payload,
                origin_instance_id as "origin_instance_id: InstanceId",
                deliver_to_origin
            FROM event_outbox
            WHERE created_at >= $1 AND (origin_instance_id <> $2 OR deliver_to_origin)
            ORDER BY created_at, id
            LIMIT $3
            "#,
            since,
            self.instance_id.as_str(),
            limit,
        )
        .fetch_all(&self.pool)
        .await
    }

    pub(super) async fn now(&self) -> Result<chrono::DateTime<chrono::Utc>, sqlx::Error> {
        sqlx::query_scalar!(r#"SELECT now() AS "now!""#)
            .fetch_one(&self.pool)
            .await
    }

    pub(super) async fn prune(
        &self,
        cutoff: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, sqlx::Error> {
        durable::DurableOutboxRepository::purge_processed_before(
            &self.pool,
            cutoff,
            PRUNE_BATCH_ROWS,
        )
        .await
    }
}
