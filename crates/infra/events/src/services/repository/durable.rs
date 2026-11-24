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
use systemprompt_identifiers::{Actor, EventOutboxId, InstanceId, OutboxAggregateKey};

use crate::services::durable::ReportingFactRaw;
use crate::services::routing::{NOTIFICATION_FACTS_CHANNEL, OUTBOX_CHANNEL, OutboxChannel};

#[derive(Debug)]
pub(crate) struct FactRow {
    pub id: EventOutboxId,
    // JSON: versioned facts are decoded by the registered consumer.
    pub fact: serde_json::Value,
}

#[derive(Debug)]
pub(crate) struct ReplayRow {
    pub id: EventOutboxId,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub fact: sqlx::types::Json<ReportingFactRaw>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ReplayQuery<'a> {
    pub consumer: &'a str,
    pub since: chrono::DateTime<chrono::Utc>,
    pub until: chrono::DateTime<chrono::Utc>,
    pub limit: i64,
    pub after: Option<&'a EventOutboxId>,
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
    pub aggregate: Option<&'a OutboxAggregateKey>,
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
              consumer, fact, deliver_to_origin, aggregate_key, aggregate_seq) \
             SELECT $1,$2,$3,$4,$5,$6,$7,$8,$9,TRUE,$10::text, \
                    CASE WHEN $10::text = '' THEN 0 ELSE COALESCE( \
                        (SELECT max(e.aggregate_seq) FROM event_outbox e \
                         WHERE e.consumer = $8 AND e.aggregate_key = $10::text), 0) + 1 END",
            row.id as _,
            row.channel.as_str(),
            row.actor.user_id as _,
            row.payload,
            actor_kind,
            actor_id,
            row.origin as _,
            row.consumer,
            row.fact,
            row.aggregate.map_or("", OutboxAggregateKey::as_str)
        )
        .execute(&mut *conn)
        .await?;
        let wake = if matches!(row.channel, OutboxChannel::Fact) {
            NOTIFICATION_FACTS_CHANNEL
        } else {
            OUTBOX_CHANNEL
        };
        sqlx::query!("SELECT pg_notify($1, $2)", wake, row.id as _)
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
        sqlx::query_as!(
            FactRow,
            r#"SELECT o.id AS "id: EventOutboxId", o.fact AS "fact!" FROM event_outbox o
             WHERE o.consumer = $1 AND o.processed_at IS NULL AND o.id <> ALL($3)
               AND (o.aggregate_key = '' OR NOT EXISTS (
                   SELECT 1 FROM event_outbox e
                   WHERE e.consumer = o.consumer AND e.aggregate_key = o.aggregate_key
                     AND e.processed_at IS NULL
                     AND (e.aggregate_seq, e.created_at, e.id)
                         < (o.aggregate_seq, o.created_at, o.id)))
             ORDER BY o.created_at, o.id LIMIT $2 FOR UPDATE OF o SKIP LOCKED"#,
            consumer,
            limit,
            skipped as _
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
            r#"SELECT o.id AS "id: EventOutboxId", o.fact AS "fact!" FROM event_outbox o
             WHERE o.consumer = $1 AND o.processed_at IS NULL
               AND (o.aggregate_key = '' OR NOT EXISTS (
                   SELECT 1 FROM event_outbox e
                   WHERE e.consumer = o.consumer AND e.aggregate_key = o.aggregate_key
                     AND e.processed_at IS NULL
                     AND (e.aggregate_seq, e.created_at, e.id)
                         < (o.aggregate_seq, o.created_at, o.id)))
             ORDER BY o.created_at, o.id LIMIT 1 FOR UPDATE OF o SKIP LOCKED"#,
            consumer
        )
        .fetch_optional(&mut *conn)
        .await
    }

    pub(crate) async fn pending(
        pool: &PgPool,
        consumer: &str,
    ) -> Result<(i64, Option<i64>), sqlx::Error> {
        let pending = sqlx::query_scalar!(
            r#"SELECT COALESCE(SUM(delta), 0)::BIGINT AS "pending!"
               FROM event_outbox_backlog WHERE consumer = $1"#,
            consumer
        )
        .fetch_one(pool)
        .await?;
        let oldest = sqlx::query_scalar!(
            r#"SELECT EXTRACT(EPOCH FROM now() - created_at)::BIGINT AS "oldest!"
               FROM event_outbox
               WHERE consumer = $1 AND processed_at IS NULL
               ORDER BY created_at, id LIMIT 1"#,
            consumer
        )
        .fetch_optional(pool)
        .await?;
        Ok((pending, oldest))
    }

    pub(crate) async fn mark_processed(
        conn: &mut PgConnection,
        id: &EventOutboxId,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE event_outbox SET processed_at = now() WHERE id = $1",
            id as _
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

    pub(crate) async fn replay_window(
        pool: &PgPool,
        query: ReplayQuery<'_>,
    ) -> Result<Vec<ReplayRow>, sqlx::Error> {
        sqlx::query_as!(
            ReplayRow,
            r#"SELECT o.id AS "id: EventOutboxId", o.created_at, o.fact AS "fact!: sqlx::types::Json<ReportingFactRaw>"
             FROM event_outbox o
             WHERE o.consumer = $1 AND o.fact IS NOT NULL
               AND o.created_at >= $2 AND o.created_at < $3
               AND ($5::text IS NULL OR (o.created_at, o.id) > (
                   SELECT a.created_at, a.id FROM event_outbox a WHERE a.id = $5))
             ORDER BY o.created_at, o.id LIMIT $4"#,
            query.consumer,
            query.since,
            query.until,
            query.limit,
            query.after as _,
        )
        .fetch_all(pool)
        .await
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
