//! A claimed outbox event as the brain pipeline sees it: the event, its
//! pathway and that pathway's weight.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::EventOutboxId;

use super::event::{BrainEvent, BrainEventKind};

/// The route an event takes into the brain.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Pathway {
    DocumentRevision,
    CommentResolved,
    Entered,
}

impl Pathway {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DocumentRevision => "document_revision",
            Self::CommentResolved => "comment_resolved",
            Self::Entered => "entered",
        }
    }

    #[must_use]
    pub fn from_kind(kind: &BrainEventKind) -> Option<Self> {
        match kind.as_str() {
            BrainEventKind::REVISION_LANDED => Some(Self::DocumentRevision),
            BrainEventKind::COMMENT_RESOLVED => Some(Self::CommentResolved),
            BrainEventKind::FACT_ENTERED => Some(Self::Entered),
            _ => None,
        }
    }

    #[must_use]
    pub const fn default_weight(self) -> PathwayWeight {
        match self {
            Self::DocumentRevision | Self::CommentResolved | Self::Entered => PathwayWeight::High,
        }
    }
}

/// How urgently a pathway's events are processed: `High` immediately,
/// `Medium` and `Low` in scheduled batches, `None` never.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PathwayWeight {
    None,
    Low,
    Medium,
    High,
}

/// One claimed outbox event, normalised for the filters and extractors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BrainEnvelope {
    pub outbox_id: EventOutboxId,
    pub event: BrainEvent,
    pub pathway: Pathway,
    pub weight: PathwayWeight,
    pub received_at: DateTime<Utc>,
}

impl BrainEnvelope {
    #[must_use]
    pub fn new(
        outbox_id: EventOutboxId,
        event: BrainEvent,
        received_at: DateTime<Utc>,
    ) -> Option<Self> {
        let pathway = Pathway::from_kind(&event.kind)?;
        Some(Self {
            outbox_id,
            event,
            pathway,
            weight: pathway.default_weight(),
            received_at,
        })
    }
}
