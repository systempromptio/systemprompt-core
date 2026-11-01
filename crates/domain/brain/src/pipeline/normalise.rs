//! The normalise stage: an outbox event becomes a [`Normalised`] event, its
//! envelope with the pathway taken from its kind and its payload decoded
//! into the shape that pathway carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{EventOutboxId, UserId};
use systemprompt_models::brain::{
    BrainEnvelope, BrainEvent, CommentResolved, EnteredFact, FactSource, Pathway, RevisionLanded,
};

use crate::error::{BrainError, BrainResult};

/// The decoded payload of a brain event.
#[derive(Debug, Clone, PartialEq)]
pub enum EventPayload {
    Revision(RevisionLanded),
    Comment(CommentResolved),
    Entered(EnteredFact),
}

/// An event ready for the pipeline: its envelope and typed payload.
#[derive(Debug, Clone, PartialEq)]
pub struct Normalised {
    pub envelope: BrainEnvelope,
    pub payload: EventPayload,
}

impl Normalised {
    #[must_use]
    pub const fn pathway(&self) -> Pathway {
        self.envelope.pathway
    }

    #[must_use]
    pub const fn actor(&self) -> &UserId {
        &self.envelope.event.actor
    }

    #[must_use]
    pub fn source(&self) -> FactSource {
        match &self.payload {
            EventPayload::Revision(landed) => FactSource::Document {
                document_id: landed.document_id.clone(),
                revision_id: landed.revision_id.clone(),
                span: None,
            },
            EventPayload::Comment(resolved) => FactSource::Comment {
                document_id: resolved.document_id.clone(),
                comment_id: resolved.comment_id.clone(),
            },
            EventPayload::Entered(_) => FactSource::EnteredBy {
                user_id: self.envelope.event.actor.clone(),
            },
        }
    }
}

pub fn normalise(
    outbox_id: EventOutboxId,
    event: BrainEvent,
    received_at: DateTime<Utc>,
) -> BrainResult<Normalised> {
    let kind = event.kind.to_string();
    let envelope = BrainEnvelope::new(outbox_id, event, received_at)
        .ok_or_else(|| BrainError::UnknownEvent(kind))?;
    let payload = envelope.event.payload.clone();
    let payload = match envelope.pathway {
        Pathway::DocumentRevision => EventPayload::Revision(serde_json::from_value(payload)?),
        Pathway::CommentResolved => EventPayload::Comment(serde_json::from_value(payload)?),
        Pathway::Entered => EventPayload::Entered(serde_json::from_value(payload)?),
    };
    Ok(Normalised { envelope, payload })
}
