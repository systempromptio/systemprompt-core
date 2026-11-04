//! Decoding the claimed rows of a drain round before it starts: the fact
//! envelope first, its data only after the version is checked, so an event
//! this build cannot read is recorded as a decode failure and never claimed
//! in the result ledger.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_events::services::durable::ReportingFact;
use systemprompt_identifiers::EventOutboxId;
use systemprompt_models::brain::{BRAIN_EVENT_VERSION, BrainEvent};

#[derive(Debug, thiserror::Error)]
pub(super) enum Undecodable {
    #[error("brain event does not decode")]
    Json(#[from] serde_json::Error),
    #[error("unsupported event version {0}")]
    Version(u32),
}

pub(super) struct Decoded {
    pub(super) id: EventOutboxId,
    pub(super) event: Result<BrainEvent, Undecodable>,
}

// JSON: the fact envelope is decoded first, and its data only after its
// version is checked.
fn decode(raw: &serde_json::Value) -> Result<BrainEvent, Undecodable> {
    let fact: ReportingFact<serde_json::Value> = serde_json::from_value(raw.clone())?;
    if fact.version != BRAIN_EVENT_VERSION {
        return Err(Undecodable::Version(fact.version));
    }
    Ok(serde_json::from_value(fact.data)?)
}

// JSON: the claimed rows carry the raw outbox envelope; decode is the one
// place it becomes a typed BrainEvent.
pub(super) fn decode_all(entries: Vec<(EventOutboxId, serde_json::Value)>) -> Vec<Decoded> {
    entries
        .into_iter()
        .map(|(id, raw)| Decoded {
            id,
            event: decode(&raw),
        })
        .collect()
}
