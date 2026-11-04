//! What one round remembers of its model calls, per event and kind.
//!
//! A round that rolls its per-event writes back and runs its events again
//! never calls the model twice for one `(event, kind)`: each extraction
//! is remembered the moment it returns, with the candidates it produced
//! and the spend row that was recorded, and the second pass re-records
//! that row and reuses the candidates. A kind whose extraction failed was
//! never remembered and is extracted (and paid for) on the second pass as
//! it would be on any retry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use systemprompt_identifiers::{EventOutboxId, FactKindId};
use systemprompt_models::brain::FactCandidate;

use crate::models::NewExtraction;

/// One `(event, kind)` extraction as the first pass produced it.
#[derive(Debug, Clone)]
pub struct Remembered {
    pub candidates: Vec<FactCandidate>,
    pub record: NewExtraction,
}

#[derive(Debug, Default)]
pub(super) struct Extractions {
    inner: Mutex<HashMap<(EventOutboxId, FactKindId), Remembered>>,
}

impl Extractions {
    pub(super) fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub(super) fn remember(&self, id: &EventOutboxId, kind: &FactKindId, remembered: Remembered) {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert((id.clone(), kind.clone()), remembered);
    }

    pub(super) fn recall(&self, id: &EventOutboxId, kind: &FactKindId) -> Option<Remembered> {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&(id.clone(), kind.clone()))
            .cloned()
    }
}
