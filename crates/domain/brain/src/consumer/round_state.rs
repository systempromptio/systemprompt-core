//! [`RoundState`]: what the consumer remembers between drain rounds.
//!
//! It holds back the events that failed transiently until the poll interval
//! has passed, so a batch round that keeps going after a transient failure
//! does not re-claim the same event at once and spend its attempts in a
//! burst. It records when the last round ran, for health, and when the
//! store gauges were last refreshed, so a busy consumer refreshes them at
//! most once per [`GAUGE_REFRESH`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use systemprompt_identifiers::EventOutboxId;

pub(crate) const GAUGE_REFRESH: Duration = Duration::from_secs(5);

#[derive(Debug, Default)]
pub(crate) struct RoundState {
    held: Mutex<HashMap<EventOutboxId, Instant>>,
    last_round: Mutex<Option<DateTime<Utc>>>,
    gauges_at: Mutex<Option<Instant>>,
}

impl RoundState {
    pub(crate) fn held(&self) -> Vec<EventOutboxId> {
        let now = Instant::now();
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        held.retain(|_, until| *until > now);
        held.keys().cloned().collect()
    }

    pub(crate) fn hold(&self, ids: Vec<EventOutboxId>, wait: Duration) {
        if ids.is_empty() {
            return;
        }
        let until = Instant::now() + wait;
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        held.extend(ids.into_iter().map(|id| (id, until)));
    }

    pub(crate) fn ran(&self) {
        *self
            .last_round
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(Utc::now());
    }

    pub(crate) fn last_round(&self) -> Option<DateTime<Utc>> {
        *self
            .last_round
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn gauges_due(&self) -> bool {
        let now = Instant::now();
        let mut at = self
            .gauges_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if at.is_some_and(|last| now.duration_since(last) < GAUGE_REFRESH) {
            return false;
        }
        *at = Some(now);
        true
    }
}
