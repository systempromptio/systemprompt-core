//! [`OutboxNotificationSink`]: the production [`NotificationSink`].
//!
//! It appends the event as a durable fact for the notifications consumer on the
//! producer's own connection, so the event commits or rolls back with the
//! change that caused it.
//!
//! The row is fact-only: it carries no SSE payload, so nothing reaches the
//! actor's stream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_events::services::durable::{
    DurableEventError, DurableOutbox, ReportingFact, SseEvent,
};
use systemprompt_extension::notifications::{
    DynNotificationSink, NotificationSink, NotificationSinkError,
};
use systemprompt_identifiers::{Actor, InstanceId};
use systemprompt_models::notifications::{
    NOTIFICATION_EVENT_VERSION, NOTIFICATIONS_CONSUMER, NotificationEvent,
};
use systemprompt_traits::RepositoryError;

/// Appends notification events to the durable outbox.
#[derive(Debug, Clone)]
pub struct OutboxNotificationSink {
    outbox: Arc<DurableOutbox>,
}

impl OutboxNotificationSink {
    #[must_use]
    pub const fn new(outbox: Arc<DurableOutbox>) -> Self {
        Self { outbox }
    }

    #[must_use]
    pub fn over_write_pool(database: &DbPool, instance_id: &InstanceId) -> DynNotificationSink {
        let outbox = DurableOutbox::new(
            database.write_pool().as_ref().clone(),
            systemprompt_events::process_origin(instance_id),
        );
        Arc::new(Self::new(Arc::new(outbox)))
    }
}

#[async_trait]
impl NotificationSink for OutboxNotificationSink {
    fn is_available(&self) -> bool {
        true
    }

    async fn emit(
        &self,
        conn: &mut sqlx::PgConnection,
        event: &NotificationEvent,
    ) -> Result<(), NotificationSinkError> {
        let fact = ReportingFact {
            consumer: NOTIFICATIONS_CONSUMER.to_owned(),
            kind: event.kind.as_str().to_owned(),
            version: NOTIFICATION_EVENT_VERSION,
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
                DurableEventError::Database(e) => NotificationSinkError::Database(e),
                DurableEventError::Serialization(e) => NotificationSinkError::Encode(e),
                DurableEventError::InvalidContract => NotificationSinkError::Repository(
                    RepositoryError::invalid_argument("fact", "outbox contract rejected the fact"),
                ),
            })?;
        Ok(())
    }
}
