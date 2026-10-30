//! Awareness on a document's live topic, gated to one publish per
//! [`AWARENESS_INTERVAL`] per participant on each replica.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::PoisonError;
use std::time::Instant;

use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_models::agui::CustomPayload;
use systemprompt_models::documents::DocumentAwarenessEvent;

use super::{AWARENESS_GATE_PRUNE_AT, AWARENESS_INTERVAL, DocumentEvents};
use crate::documents::error::{DocumentsError, EventFailure};

impl DocumentEvents {
    pub async fn awareness(&self, event: &DocumentAwarenessEvent) -> Result<bool, DocumentsError> {
        if !self.awareness_due(&event.document_id, &event.user_id) {
            return Ok(false);
        }
        let name = DocumentAwarenessEvent::NAME;
        let value = serde_json::to_value(event).map_err(|source| DocumentsError::Events {
            event: name,
            source: EventFailure::Serialize(source),
        })?;
        self.publish_topic(name, &event.user_id, value, None)
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
}
