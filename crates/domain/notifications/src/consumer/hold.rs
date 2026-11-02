//! [`RetryHold`]: facts that failed transiently, held back from the next
//! claims until the poll interval has passed, so a batch round that keeps
//! going after a transient failure does not re-claim the same fact at once
//! and spend its attempts in a burst.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use systemprompt_identifiers::EventOutboxId;

#[derive(Debug, Default)]
pub(crate) struct RetryHold(Mutex<HashMap<EventOutboxId, Instant>>);

impl RetryHold {
    /// The facts still held back; expired holds are dropped.
    pub(crate) fn held(&self) -> Vec<EventOutboxId> {
        let now = Instant::now();
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        held.retain(|_, until| *until > now);
        held.keys().cloned().collect()
    }

    pub(crate) fn hold(&self, ids: Vec<EventOutboxId>, wait: Duration) {
        if ids.is_empty() {
            return;
        }
        let until = Instant::now() + wait;
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        held.extend(ids.into_iter().map(|id| (id, until)));
    }
}
