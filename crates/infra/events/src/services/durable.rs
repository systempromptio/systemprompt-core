//! Transactional facts carried alongside existing SSE payloads in the outbox.
//!
//! Each row has one durable consumer. Processing holds a row lock and exposes
//! the same transaction for projection writes and acknowledgement. Dropping a
//! delivery rolls it back. Consumers must poll pending rows after notification
//! loss; SSE itself retains its live-only delivery semantics. A bridge must run
//! on the emitting instance to deliver transactionally appended SSE events.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use systemprompt_identifiers::{Actor, EventOutboxId, InstanceId};
use systemprompt_models::{A2AEvent, AgUiEvent, AnalyticsEvent, SystemEvent};

use super::routing::{OUTBOX_CHANNEL, OutboxChannel};

#[derive(Debug, thiserror::Error)]
pub enum DurableEventError {
    #[error("durable event consumer and kind must be nonempty and version must be positive")]
    InvalidContract,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReportingFact<T> {
    pub consumer: String,
    pub kind: String,
    pub version: u32,
    pub data: T,
}

#[derive(Debug)]
pub enum SseEvent<'a> {
    AgUi(&'a AgUiEvent),
    A2A(&'a A2AEvent),
    System(&'a SystemEvent),
    Analytics(&'a AnalyticsEvent),
}

impl SseEvent<'_> {
    // JSON: the existing SSE channel determines the wire payload type.
    fn encode(&self) -> Result<(OutboxChannel, serde_json::Value), serde_json::Error> {
        Ok(match self {
            Self::AgUi(event) => (OutboxChannel::AgUi, serde_json::to_value(event)?),
            Self::A2A(event) => (OutboxChannel::A2A, serde_json::to_value(event)?),
            Self::System(event) => (OutboxChannel::System, serde_json::to_value(event)?),
            Self::Analytics(event) => (OutboxChannel::Analytics, serde_json::to_value(event)?),
        })
    }
}

#[derive(Debug, Clone)]
pub struct DurableOutbox {
    pool: PgPool,
    instance_id: InstanceId,
}

impl DurableOutbox {
    pub const fn new(pool: PgPool, instance_id: InstanceId) -> Self {
        Self { pool, instance_id }
    }

    pub async fn prune_processed_before(
        &self,
        cutoff: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, sqlx::Error> {
        prune_processed(&self.pool, cutoff).await
    }

    pub async fn append<T: Serialize + Sync>(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        actor: &Actor,
        event: SseEvent<'_>,
        fact: &ReportingFact<T>,
    ) -> Result<EventOutboxId, DurableEventError> {
        if fact.consumer.trim().is_empty() || fact.kind.trim().is_empty() || fact.version == 0 {
            return Err(DurableEventError::InvalidContract);
        }
        let (channel, payload) = event.encode()?;
        let encoded_fact = serde_json::to_value(fact)?;
        let id = EventOutboxId::generate();
        let (actor_kind, actor_id) = actor.audit_columns();
        sqlx::query!(
            "INSERT INTO event_outbox \
             (id, channel, user_id, payload, actor_kind, actor_id, origin_instance_id, \
              consumer, fact, deliver_to_origin) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,TRUE)",
            id.as_str(),
            channel.as_str(),
            actor.user_id.as_str(),
            payload,
            actor_kind,
            actor_id,
            self.instance_id.as_str(),
            &fact.consumer,
            encoded_fact
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!("SELECT pg_notify($1, $2)", OUTBOX_CHANNEL, id.as_str())
            .fetch_one(&mut **tx)
            .await?;
        Ok(id)
    }
}

/// Claim-only side of the outbox: a consumer never appends, so it carries no
/// emitting instance identity.
#[derive(Debug, Clone)]
pub struct OutboxConsumer {
    pool: PgPool,
}

impl OutboxConsumer {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn claim(&self, consumer: &str) -> Result<Option<Delivery>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_as!(
            FactRow,
            r#"SELECT id AS "id: EventOutboxId", fact AS "fact!" FROM event_outbox
             WHERE consumer = $1 AND processed_at IS NULL
             ORDER BY created_at, id LIMIT 1 FOR UPDATE SKIP LOCKED"#,
            consumer
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(row) = row {
            Ok(Some(Delivery { tx, row }))
        } else {
            tx.rollback().await?;
            Ok(None)
        }
    }
}

pub(super) async fn prune_processed(
    pool: &PgPool,
    cutoff: chrono::DateTime<chrono::Utc>,
) -> Result<u64, sqlx::Error> {
    sqlx::query!(
        "DELETE FROM event_outbox WHERE created_at < $1 AND (consumer IS NULL OR processed_at IS NOT NULL)",
        cutoff
    )
        .execute(pool)
        .await
        .map(|result| result.rows_affected())
}

#[derive(Debug)]
struct FactRow {
    id: EventOutboxId,
    // JSON: versioned reporting facts are decoded by the registered consumer.
    fact: serde_json::Value,
}

#[must_use]
#[derive(Debug)]
pub struct Delivery {
    tx: Transaction<'static, Postgres>,
    row: FactRow,
}

impl Delivery {
    pub fn id(&self) -> EventOutboxId {
        self.row.id.clone()
    }

    pub fn fact<T: DeserializeOwned>(&self) -> Result<ReportingFact<T>, serde_json::Error> {
        serde_json::from_value(self.row.fact.clone())
    }

    pub fn connection(&mut self) -> &mut PgConnection {
        &mut self.tx
    }

    pub async fn acknowledge(mut self) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE event_outbox SET processed_at = now() WHERE id = $1",
            self.row.id.as_str()
        )
        .execute(&mut *self.tx)
        .await?;
        self.tx.commit().await
    }

    pub async fn rollback(self) -> Result<(), sqlx::Error> {
        self.tx.rollback().await
    }
}
