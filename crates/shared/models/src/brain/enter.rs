//! A fact a person types in, carried as the payload of a
//! `brain.fact.entered` event.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{FactKindId, TopicId, UserId};

use super::event::{BrainEvent, BrainEventKind};
use super::scope::FactScope;
use super::subject::SubjectKey;

/// The status an entered fact starts in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnteredStatus {
    #[default]
    Candidate,
    Proposed,
}

/// A person-entered fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnteredFact {
    pub kind: FactKindId,
    pub subject: SubjectKey,
    // JSON: entered fields are checked against the kind declaration by the pipeline.
    pub fields: serde_json::Map<String, serde_json::Value>,
    pub scope: FactScope,
    #[serde(default)]
    pub topics: Vec<TopicId>,
    #[serde(default)]
    pub status: EnteredStatus,
}

impl EnteredFact {
    pub fn to_event(
        &self,
        actor: UserId,
        occurred_at: DateTime<Utc>,
    ) -> Result<BrainEvent, serde_json::Error> {
        Ok(
            BrainEvent::new(BrainEventKind::fact_entered(), actor, occurred_at)
                .with_payload(serde_json::to_value(self)?),
        )
    }
}
