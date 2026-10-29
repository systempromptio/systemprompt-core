//! The inbox shared between the inbox task and the GUI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;
use systemprompt_identifiers::NotificationId;
use systemprompt_models::notifications::{MarkRead, MarkSeen};
use tokio::sync::{mpsc, watch};

use super::state::InboxState;
use super::toast::InboxToast;

const REQUEST_CAPACITY: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboxRequest {
    Refresh,
    LoadOlder,
    MarkRead(MarkRead),
    MarkSeen(MarkSeen),
    Escalate {
        id: NotificationId,
        note: Option<String>,
    },
}

pub type InboxRequestReceiver = mpsc::Receiver<InboxRequest>;

/// The inbox every reader sees, shared by the inbox task and the GUI.
///
/// Writes go through [`InboxHandle::update`], which persists the state and
/// bumps the generation the GUI watches; the GUI asks for gateway work through
/// [`InboxHandle::request`] and never holds a bearer itself. A gateway built
/// without notifications answers the feed with 404; the handle then reports
/// itself unavailable until the next stream open retries. Seen receipts are
/// claimed once per id per process through [`InboxHandle::claim_unseen`].
#[derive(Debug)]
pub struct InboxHandle {
    state: Mutex<InboxState>,
    toasts: Mutex<Vec<InboxToast>>,
    last_error: Mutex<Option<String>>,
    available: AtomicBool,
    seen: Mutex<HashSet<NotificationId>>,
    generation: watch::Sender<u64>,
    requests: mpsc::Sender<InboxRequest>,
    path: Option<PathBuf>,
}

impl InboxHandle {
    #[must_use]
    pub fn new(path: Option<PathBuf>) -> (Arc<Self>, InboxRequestReceiver) {
        let state = path.as_deref().map_or_else(InboxState::default, |path| {
            InboxState::load(path).unwrap_or_else(|error| {
                tracing::warn!(%error, "inbox state unreadable; starting empty");
                InboxState::default()
            })
        });
        let (requests, rx) = mpsc::channel(REQUEST_CAPACITY);
        let handle = Self {
            state: Mutex::new(state),
            toasts: Mutex::new(Vec::new()),
            last_error: Mutex::new(None),
            available: AtomicBool::new(true),
            seen: Mutex::new(HashSet::new()),
            generation: watch::Sender::new(0),
            requests,
            path,
        };
        (Arc::new(handle), rx)
    }

    #[must_use]
    pub fn snapshot(&self) -> InboxState {
        self.state.lock().clone()
    }

    #[must_use]
    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().clone()
    }

    #[must_use]
    pub fn available(&self) -> bool {
        self.available.load(Ordering::Acquire)
    }

    pub fn set_available(&self, available: bool) {
        if self.available.swap(available, Ordering::AcqRel) != available {
            self.bump();
        }
    }

    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.generation.subscribe()
    }

    pub fn update<R>(&self, change: impl FnOnce(&mut InboxState) -> R) -> R {
        let (result, saved) = {
            let mut guard = self.state.lock();
            let state: &mut InboxState = &mut guard;
            let before = state.clone();
            let result = change(state);
            let saved = (*state != before).then(|| state.clone());
            drop(guard);
            (result, saved)
        };
        if let Some(saved) = saved {
            if let Some(path) = &self.path
                && let Err(error) = saved.save(path)
            {
                tracing::warn!(%error, "inbox state not persisted; it is rebuilt on the next catch-up");
            }
            self.bump();
        }
        result
    }

    #[must_use]
    pub fn claim_unseen(&self, shown: &[NotificationId]) -> Vec<NotificationId> {
        let held: Vec<NotificationId> = {
            let state = self.state.lock();
            shown
                .iter()
                .filter(|id| state.find(id).is_some())
                .cloned()
                .collect()
        };
        let mut seen = self.seen.lock();
        held.into_iter()
            .filter(|id| seen.insert(id.clone()))
            .collect()
    }

    pub fn release_seen(&self, ids: &[NotificationId]) {
        let mut seen = self.seen.lock();
        for id in ids {
            seen.remove(id);
        }
    }

    pub fn record_error(&self, error: Option<String>) {
        let changed = {
            let mut slot = self.last_error.lock();
            let changed = *slot != error;
            *slot = error;
            changed
        };
        if changed {
            self.bump();
        }
    }

    pub fn push_toasts(&self, toasts: Vec<InboxToast>) {
        if toasts.is_empty() {
            return;
        }
        self.toasts.lock().extend(toasts);
        self.bump();
    }

    #[must_use]
    pub fn take_toasts(&self) -> Vec<InboxToast> {
        std::mem::take(&mut *self.toasts.lock())
    }

    pub fn request(&self, request: InboxRequest) {
        if let Err(error) = self.requests.try_send(request) {
            tracing::warn!(%error, "inbox request not queued");
        }
    }

    fn bump(&self) {
        self.generation.send_modify(|g| *g = g.wrapping_add(1));
    }
}
