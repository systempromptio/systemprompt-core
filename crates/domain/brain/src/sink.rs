//! [`OutboxBrainSink`]: the production [`BrainSink`].
//!
//! It appends the event as a durable fact for the brain consumer on the
//! producer's own connection, so the event commits or rolls back with the
//! change that caused it. The row is fact-only: it carries no SSE payload.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_events::services::durable::{
    DurableEventError, DurableOutbox, ReportingFact, SseEvent,
};
use systemprompt_extension::brain::{BrainSink, BrainSinkError};
use systemprompt_identifiers::{Actor, EventOutboxId};
use systemprompt_models::brain::{BRAIN_CONSUMER, BRAIN_EVENT_VERSION, BrainEvent};
use systemprompt_traits::RepositoryError;

/// Appends brain events to the durable outbox.
#[derive(Debug, Clone)]
pub struct OutboxBrainSink {
    outbox: Arc<DurableOutbox>,
}

impl OutboxBrainSink {
    #[must_use]
    pub const fn new(outbox: Arc<DurableOutbox>) -> Self {
        Self { outbox }
    }
}

#[async_trait]
impl BrainSink for OutboxBrainSink {
    fn is_available(&self) -> bool {
        true
    }

    async fn emit(
        &self,
        conn: &mut sqlx::PgConnection,
        event: &BrainEvent,
    ) -> Result<EventOutboxId, BrainSinkError> {
        let fact = ReportingFact {
            consumer: BRAIN_CONSUMER.to_owned(),
            kind: event.kind.as_str().to_owned(),
            version: BRAIN_EVENT_VERSION,
            data: event,
        };
        self.outbox
            .append_on(
                conn,
                &Actor::user(event.actor.clone()),
                SseEvent::FactOnly,
                &fact,
            )
            .await
            .map_err(|error| match error {
                DurableEventError::Database(e) => BrainSinkError::Database(e),
                DurableEventError::Serialization(e) => BrainSinkError::Encode(e),
                DurableEventError::InvalidContract => BrainSinkError::Repository(
                    RepositoryError::invalid_argument("fact", "outbox contract rejected the fact"),
                ),
            })
    }
}
