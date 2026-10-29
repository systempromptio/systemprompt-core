//! Queries behind the transactional outbox: append a fact-bearing row, claim
//! pending rows under a row lock, acknowledge them, and prune processed rows.
//! Every claim and acknowledgement runs on the caller's transaction so the
//! projection writes and the acknowledgement commit together.
//!
//! An append wakes exactly one listener channel: a fact-only row notifies
//! [`NOTIFICATION_FACTS_CHANNEL`] (the durable consumers), a row with a live
//! frame notifies [`OUTBOX_CHANNEL`] (the cross-replica relay), so neither
//! listener wakes for the other's rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{PgConnection, PgPool};
use systemprompt_identifiers::{Actor, EventOutboxId, InstanceId};

use crate::services::routing::{NOTIFICATION_FACTS_CHANNEL, OUTBOX_CHANNEL, OutboxChannel};

#[derive(Debug)]
pub(crate) struct FactRow {
    pub id: EventOutboxId,
    // JSON: versioned facts are decoded by the registered consumer.
    pub fact: serde_json::Value,
}

#[derive(Debug)]
pub(crate) struct DurableRow<'a> {
    pub id: &'a EventOutboxId,
    pub channel: OutboxChannel,
    pub actor: &'a Actor,
    pub origin: &'a InstanceId,
    pub consumer: &'a str,
    // JSON: SSE wire payload, polymorphic by `channel`.
    pub payload: serde_json::Value,
    // JSON: the encoded `ReportingFact` envelope.
    pub fact: serde_json::Value,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DurableOutboxRepository;

impl DurableOutboxRepository {
    pub(crate) async fn insert(
        conn: &mut PgConnection,
        row: DurableRow<'_>,
    ) -> Result<(), sqlx::Error> {
        let (actor_kind, actor_id) = row.actor.audit_columns();
        sqlx::query!(
            "INSERT INTO event_outbox \
             (id, channel, user_id, payload, actor_kind, actor_id, origin_instance_id, \
              consumer, fact, deliver_to_origin) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,TRUE)",
            row.id.as_str(),
            row.channel.as_str(),
            row.actor.user_id.as_str(),
            row.payload,
            actor_kind,
            actor_id,
            row.origin.as_str(),
            row.consumer,
            row.fact
        )
        .execute(&mut *conn)
        .await?;
        let wake = if matches!(row.channel, OutboxChannel::Fact) {
            NOTIFICATION_FACTS_CHANNEL
        } else {
            OUTBOX_CHANNEL
        };
        sqlx::query!("SELECT pg_notify($1, $2)", wake, row.id.as_str())
            .fetch_one(&mut *conn)
            .await?;
        Ok(())
    }

    pub(crate) async fn claim_batch(
        conn: &mut PgConnection,
        consumer: &str,
        limit: i64,
        skipped: &[EventOutboxId],
    ) -> Result<Vec<FactRow>, sqlx::Error> {
        let skipped: Vec<String> = skipped.iter().map(ToString::to_string).collect();
        sqlx::query_as!(
            FactRow,
            r#"SELECT id AS "id: EventOutboxId", fact AS "fact!" FROM event_outbox
             WHERE consumer = $1 AND processed_at IS NULL AND id <> ALL($3)
             ORDER BY created_at, id LIMIT $2 FOR UPDATE SKIP LOCKED"#,
            consumer,
            limit,
            &skipped
        )
        .fetch_all(&mut *conn)
        .await
    }

    pub(crate) async fn claim_one(
        conn: &mut PgConnection,
        consumer: &str,
    ) -> Result<Option<FactRow>, sqlx::Error> {
        sqlx::query_as!(
            FactRow,
            r#"SELECT id AS "id: EventOutboxId", fact AS "fact!" FROM event_outbox
             WHERE consumer = $1 AND processed_at IS NULL
             ORDER BY created_at, id LIMIT 1 FOR UPDATE SKIP LOCKED"#,
            consumer
        )
        .fetch_optional(&mut *conn)
        .await
    }

    pub(crate) async fn mark_processed(
        conn: &mut PgConnection,
        id: &EventOutboxId,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE event_outbox SET processed_at = now() WHERE id = $1",
            id.as_str()
        )
        .execute(&mut *conn)
        .await
        .map(|_| ())
    }

    pub(crate) async fn mark_all_processed(
        conn: &mut PgConnection,
        ids: impl Iterator<Item = &EventOutboxId>,
    ) -> Result<(), sqlx::Error> {
        let ids: Vec<String> = ids.map(ToString::to_string).collect();
        sqlx::query!(
            "UPDATE event_outbox SET processed_at = now() WHERE id = ANY($1)",
            &ids
        )
        .execute(&mut *conn)
        .await
        .map(|_| ())
    }

    pub(crate) async fn purge_processed_before(
        pool: &PgPool,
        cutoff: chrono::DateTime<chrono::Utc>,
        batch: i64,
    ) -> Result<u64, sqlx::Error> {
        sqlx::query!(
            "DELETE FROM event_outbox WHERE ctid = ANY(ARRAY(SELECT ctid FROM event_outbox WHERE created_at < $1 AND (consumer IS NULL OR processed_at IS NOT NULL) LIMIT $2))",
            cutoff,
            batch
        )
        .execute(pool)
        .await
        .map(|result| result.rows_affected())
    }
}
