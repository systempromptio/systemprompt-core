//! Live progress for a sync in flight.
//!
//! The sink is held by the bridge context rather than threaded through every
//! sync signature: the GUI installs one for the duration of a sync, and the
//! CLI leaves it unset, where every report is a cheap no-op.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, Mutex};

/// One step of a sync, as the user should see it.
#[derive(Debug, Clone)]
pub struct SyncProgress {
    pub phase: &'static str,
    pub item: String,
    pub current: usize,
    pub total: usize,
}

impl SyncProgress {
    #[must_use]
    pub fn new(phase: &'static str, item: impl Into<String>, current: usize, total: usize) -> Self {
        Self {
            phase,
            item: item.into(),
            current,
            total,
        }
    }

    #[must_use]
    pub fn label(&self) -> String {
        if self.total > 1 {
            format!("{} ({}/{})", self.item, self.current, self.total)
        } else {
            self.item.clone()
        }
    }
}

type Sink = Arc<dyn Fn(&SyncProgress) + Send + Sync>;

/// The installed reporter, or nothing. Cloneable and shared; `report` on an
/// empty sink costs one uncontended lock and returns.
#[derive(Clone, Default)]
pub struct SyncProgressSink {
    inner: Arc<Mutex<Option<Sink>>>,
}

impl std::fmt::Debug for SyncProgressSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncProgressSink").finish_non_exhaustive()
    }
}

impl SyncProgressSink {
    pub fn install(&self, sink: Sink) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some(sink);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = None;
        }
    }

    pub fn report(&self, progress: &SyncProgress) {
        let sink = self
            .inner
            .lock()
            .map_or_else(|_| None, |guard| guard.clone());
        if let Some(sink) = sink {
            sink(progress);
        }
    }
}
