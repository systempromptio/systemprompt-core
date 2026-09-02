//! Live progress for a sync in flight.
//!
//! `Sync now` did the whole job — fetch the signed manifest, download every
//! file of every plugin one at a time, stage, swap, then run each host
//! emitter — and reported exactly two things to the user: `syncing`, and some
//! tens of seconds later, `synced`. There was no way to tell a slow sync from a
//! stuck one, which is the only question anyone is asking while they wait.
//!
//! The sync internals are deep (`run_once` → `apply_manifest` → `apply_plugins`
//! → per-file fetch) and threading a callback down every signature would touch
//! every one of them, including the CLI's caller which has no UI. Instead the
//! sink is held by the bridge context, which is already passed the whole way
//! down: the GUI installs one for the duration of a sync, and the CLI leaves it
//! unset, where every report is a cheap no-op.
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

    // Why: deliberately infallible — progress reporting must never be able to
    // fail a sync, so a poisoned lock drops the update.
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
