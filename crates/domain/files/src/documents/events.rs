//! Document events on the AG-UI stream.
//!
//! Each event is an AG-UI `CUSTOM` event named by the payload's `NAME`
//! (`document.revision.created`, `document.presence`, `document.lock`,
//! `document.conflict`, `document.collab.update`, `document.comment.*`
//! (emitted from `comment_events`), `document.shared`,
//! `document.access_requested` and `document.ownership_transferred` (from
//! `sharing_events`), `document.proposal.*`) and routed to each
//! recipient through the [`EventRouter`], which also relays it to other
//! replicas. Clients filter the stream on the `document.` prefix.
//!
//! Every event except the per-recipient ones (`document.shared`,
//! `document.comment.mentioned`) is also published once on the document's
//! live topic ([`live_topic`]) and relayed to other replicas, so the
//! document-scoped live channel sees it whoever the recipients were; a collab
//! update carries `<session>:<seq>` as its resume id. Awareness goes to the
//! topic only, at most once per [`AWARENESS_INTERVAL`] per participant.
//!
//! Every `emit` reports a relay failure as [`DocumentsError::Events`] after
//! logging it; local delivery has already happened by then. Callers that
//! have committed state treat that error as a warning and never undo the
//! commit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use systemprompt_events::{EventRouter, RelayOutcome, TopicEvent};
use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_models::agui::{AgUiEventBuilder, CustomPayload, GenericCustomPayload};
use systemprompt_models::documents::{
    DocumentAwarenessEvent, DocumentCollabUpdateEvent, DocumentConflictEvent, DocumentLockEvent,
    DocumentPresenceEvent, DocumentProposalEvent, DocumentRevisionCreatedEvent,
};

use super::error::{DocumentsError, EventFailure};

pub const AWARENESS_INTERVAL: Duration = Duration::from_millis(100);

const LIVE_TOPIC_PREFIX: &str = "document:";

const AWARENESS_GATE_PRUNE_AT: usize = 4096;

type AwarenessGate = HashMap<(DocumentId, UserId), Instant>;

#[must_use]
pub fn live_topic(document: &DocumentId) -> String {
    format!("{LIVE_TOPIC_PREFIX}{document}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Audience {
    Recipients,
    RecipientsAndTopic,
}

/// Builds and routes the document AG-UI events; held by `DocumentService`.
#[derive(Debug, Clone)]
pub struct DocumentEvents {
    router: Arc<EventRouter>,
    awareness_gate: Arc<Mutex<AwarenessGate>>,
}

impl DocumentEvents {
    pub fn new(router: Arc<EventRouter>) -> Self {
        Self {
            router,
            awareness_gate: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[must_use]
    pub const fn router(&self) -> &Arc<EventRouter> {
        &self.router
    }

    pub async fn awareness(&self, event: &DocumentAwarenessEvent) -> Result<bool, DocumentsError> {
        if !self.awareness_due(&event.document_id, &event.user_id) {
            return Ok(false);
        }
        let name = DocumentAwarenessEvent::NAME;
        let value = serde_json::to_value(event).map_err(|source| DocumentsError::Events {
            event: name,
            source: EventFailure::Serialize(source),
        })?;
        self.publish_topic(name, &event.user_id, value)
            .await
            .map(|()| true)
    }

    fn awareness_due(&self, document: &DocumentId, user: &UserId) -> bool {
        let now = Instant::now();
        let mut gate = self
            .awareness_gate
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if gate.len() >= AWARENESS_GATE_PRUNE_AT {
            gate.retain(|_, last| now.duration_since(*last) < AWARENESS_INTERVAL);
        }
        let key = (document.clone(), user.clone());
        if gate
            .get(&key)
            .is_some_and(|last| now.duration_since(*last) < AWARENESS_INTERVAL)
        {
            return false;
        }
        gate.insert(key, now);
        true
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

    pub async fn proposal(
        &self,
        to: &[UserId],
        event: &DocumentProposalEvent,
    ) -> Result<(), DocumentsError> {
        self.emit(DocumentProposalEvent::name_of(event.status), to, event)
            .await
    }

    pub(crate) async fn emit<T: Serialize + Sync>(
        &self,
        name: &'static str,
        to: &[UserId],
        payload: &T,
    ) -> Result<(), DocumentsError> {
        self.emit_to(name, to, payload, Audience::RecipientsAndTopic)
            .await
    }

    pub(crate) async fn emit_to<T: Serialize + Sync>(
        &self,
        name: &'static str,
        to: &[UserId],
        payload: &T,
        audience: Audience,
    ) -> Result<(), DocumentsError> {
        let value = serde_json::to_value(payload).map_err(|source| DocumentsError::Events {
            event: name,
            source: EventFailure::Serialize(source),
        })?;
        if audience == Audience::RecipientsAndTopic
            && let Some(actor) = to.first()
            && let Err(error) = self.publish_topic(name, actor, value.clone()).await
        {
            tracing::warn!(event = name, error = %error, "Document event not published on its live topic");
        }
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

    // JSON: the serialized payload of the document event named by `name`.
    async fn publish_topic(
        &self,
        name: &'static str,
        actor: &UserId,
        value: serde_json::Value,
    ) -> Result<(), DocumentsError> {
        let Some(document) = value.get("document_id").and_then(serde_json::Value::as_str) else {
            return Ok(());
        };
        let event = TopicEvent {
            topic: format!("{LIVE_TOPIC_PREFIX}{document}"),
            name: name.to_owned(),
            resume_id: resume_id(name, &value),
            value,
        };
        match self.router.route_topic(actor, event).await.relay {
            RelayOutcome::Failed(error) => Err(DocumentsError::Events {
                event: name,
                source: EventFailure::Relay(error),
            }),
            RelayOutcome::Relayed | RelayOutcome::NotInstalled => Ok(()),
        }
    }
}

// JSON: a serialized document event; only collab updates carry a resume id.
fn resume_id(name: &str, value: &serde_json::Value) -> Option<String> {
    if name != DocumentCollabUpdateEvent::NAME {
        return None;
    }
    let session = value.get("edit_session_id")?.as_str()?;
    let seq = value.get("seq")?.as_i64()?;
    Some(format!("{session}:{seq}"))
}
