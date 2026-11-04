//! What one round remembers of its model calls.
//!
//! A round that rolls its per-event writes back and runs its events again
//! never calls the model twice for one event: the candidates each extraction
//! produced, the source text they cite, and the spend rows that were recorded,
//! re-recorded on the second pass so the daily budget still counts the call.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use systemprompt_identifiers::EventOutboxId;

use super::extract::Extracted;
use crate::models::NewExtraction;

/// One event's extraction as the first pass produced it.
#[derive(Debug, Clone, Default)]
pub struct Remembered {
    pub extracted: Extracted,
    pub records: Vec<NewExtraction>,
}

#[derive(Debug, Default)]
pub(super) struct Extractions {
    inner: Mutex<HashMap<EventOutboxId, Remembered>>,
}

impl Extractions {
    pub(super) fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub(super) fn remember(&self, id: &EventOutboxId, remembered: Remembered) {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id.clone(), remembered);
    }

    pub(super) fn recall(&self, id: &EventOutboxId) -> Option<Remembered> {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
            .cloned()
    }
}
