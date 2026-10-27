//! Document events on the AG-UI stream.
//!
//! Each event is an AG-UI `CUSTOM` event named by the payload's `NAME`
//! (`document.revision.created`, `document.presence`, `document.lock`,
//! `document.conflict`, `document.collab.update`) and routed to each recipient through the
//! [`EventRouter`], which also relays it to other replicas. Clients filter
//! the stream on the `document.` prefix.
//!
//! Every `emit` reports a relay failure as [`DocumentsError::Events`] after
//! logging it; local delivery has already happened by then. Callers that
//! have committed state treat that error as a warning and never undo the
//! commit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde::Serialize;
use systemprompt_events::{EventRouter, RelayOutcome};
use systemprompt_identifiers::UserId;
use systemprompt_models::agui::{AgUiEventBuilder, CustomPayload, GenericCustomPayload};
use systemprompt_models::documents::{
    DocumentCollabUpdateEvent, DocumentConflictEvent, DocumentLockEvent, DocumentPresenceEvent,
    DocumentRevisionCreatedEvent,
};

use super::error::{DocumentsError, EventFailure};

/// Builds and routes the document AG-UI events; held by `DocumentService`.
#[derive(Debug, Clone)]
pub struct DocumentEvents {
    router: Arc<EventRouter>,
}

impl DocumentEvents {
    pub const fn new(router: Arc<EventRouter>) -> Self {
        Self { router }
    }

    pub async fn revision_created(
        &self,
        to: &[UserId],
        event: &DocumentRevisionCreatedEvent,
    ) -> Result<(), DocumentsError> {
        self.emit(DocumentRevisionCreatedEvent::NAME, to, event)
            .await
    }

    pub async fn presence(
        &self,
        to: &[UserId],
        event: &DocumentPresenceEvent,
    ) -> Result<(), DocumentsError> {
        self.emit(DocumentPresenceEvent::NAME, to, event).await
    }

    pub async fn lock(
        &self,
        to: &[UserId],
        event: &DocumentLockEvent,
    ) -> Result<(), DocumentsError> {
        self.emit(DocumentLockEvent::NAME, to, event).await
    }

    pub async fn conflict(
        &self,
        to: &[UserId],
        event: &DocumentConflictEvent,
    ) -> Result<(), DocumentsError> {
        self.emit(DocumentConflictEvent::NAME, to, event).await
    }

    pub async fn collab_update(
        &self,
        to: &[UserId],
        event: &DocumentCollabUpdateEvent,
    ) -> Result<(), DocumentsError> {
        self.emit(DocumentCollabUpdateEvent::NAME, to, event).await
    }

    async fn emit<T: Serialize + Sync>(
        &self,
        name: &'static str,
        to: &[UserId],
        payload: &T,
    ) -> Result<(), DocumentsError> {
        let value = serde_json::to_value(payload).map_err(|source| DocumentsError::Events {
            event: name,
            source: EventFailure::Serialize(source),
        })?;
        let event = AgUiEventBuilder::custom(CustomPayload::Generic(GenericCustomPayload {
            name: name.to_owned(),
            value,
        }));
        let mut failure = None;
        for user in to {
            let outcome = self.router.route_agui(user, event.clone()).await;
            if let RelayOutcome::Failed(error) = outcome.relay {
                tracing::warn!(
                    event = name,
                    user_id = %user,
                    error = %error,
                    "Document event delivered locally but not relayed to other replicas"
                );
                failure.get_or_insert(error);
            }
        }
        failure.map_or(Ok(()), |error| {
            Err(DocumentsError::Events {
                event: name,
                source: EventFailure::Relay(error),
            })
        })
    }
}
