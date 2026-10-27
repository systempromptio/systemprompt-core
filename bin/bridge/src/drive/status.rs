//! The Drive status the tray, the GUI and the CLI read, and the handle the
//! sync loop is driven through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwap;
use chrono::{DateTime, Duration, Utc};
use parking_lot::Mutex;
use serde::Serialize;
use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_models::documents::{DocumentPresenceEvent, PresenceSource, PresenceState};
use tokio::sync::{Notify, watch};

use super::conflict::prune_resolved;
use super::error::DriveResult;
use super::root::DriveRoot;
use super::scan::local_changes;
pub use super::state::ConflictNotice;
use super::state::DriveState;

pub const EDITOR_TTL: Duration = Duration::seconds(90);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EditorPresence {
    pub document_id: DocumentId,
    pub path: Option<String>,
    pub user_id: UserId,
    pub source_kind: PresenceSource,
    pub state: PresenceState,
    pub seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DriveStatus {
    pub enabled: bool,
    pub root: Option<PathBuf>,
    pub synced_at: Option<DateTime<Utc>>,
    pub pending_uploads: usize,
    pub conflicts: Vec<ConflictNotice>,
    pub editors: Vec<EditorPresence>,
    pub last_error: Option<String>,
}

impl DriveStatus {
    pub fn from_disk(root: &DriveRoot) -> DriveResult<Self> {
        if !root.dir().is_dir() {
            return Ok(Self::default());
        }
        let mut state = DriveState::load(&root.state_path())?;
        prune_resolved(root, &mut state);
        Ok(Self {
            enabled: true,
            root: Some(root.dir().to_path_buf()),
            synced_at: None,
            pending_uploads: local_changes(root, &state)?.len(),
            conflicts: state.conflicts,
            editors: Vec::new(),
            last_error: None,
        })
    }

    #[must_use]
    pub fn editing_count(&self) -> usize {
        self.editors
            .iter()
            .filter(|e| e.state == PresenceState::Editing)
            .count()
    }
}

/// Shared between the sync loop and everything that reports on or prods it.
///
/// `cycle` serialises sync cycles: the loop, a GUI "sync now" and anything
/// else that runs one take it, so two cycles never move the same file.
#[derive(Debug)]
pub struct DriveHandle {
    status: ArcSwap<DriveStatus>,
    wake: Notify,
    touched: Mutex<BTreeSet<PathBuf>>,
    generation: watch::Sender<u64>,
    pub(crate) cycle: tokio::sync::Mutex<()>,
}

impl Default for DriveHandle {
    fn default() -> Self {
        Self {
            status: ArcSwap::from_pointee(DriveStatus::default()),
            wake: Notify::new(),
            touched: Mutex::new(BTreeSet::new()),
            generation: watch::Sender::new(0),
            cycle: tokio::sync::Mutex::new(()),
        }
    }
}

impl DriveHandle {
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    #[must_use]
    pub fn status(&self) -> Arc<DriveStatus> {
        self.status.load_full()
    }

    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.generation.subscribe()
    }

    pub fn update(&self, change: impl FnOnce(&mut DriveStatus)) {
        let mut next = (*self.status.load_full()).clone();
        change(&mut next);
        if *self.status.load_full() != next {
            self.status.store(Arc::new(next));
            self.generation.send_modify(|g| *g = g.wrapping_add(1));
        }
    }

    pub fn sync_now(&self) {
        self.wake.notify_one();
    }

    pub async fn woken(&self) {
        self.wake.notified().await;
    }

    pub fn touch(&self, path: PathBuf) {
        self.touched.lock().insert(path);
    }

    #[must_use]
    pub fn take_touched(&self) -> BTreeSet<PathBuf> {
        std::mem::take(&mut *self.touched.lock())
    }

    pub fn observe_presence(&self, event: &DocumentPresenceEvent, path: Option<String>) {
        let now = Utc::now();
        self.update(|status| {
            status
                .editors
                .retain(|e| !(e.document_id == event.document_id && e.user_id == event.user_id));
            status.editors.push(EditorPresence {
                document_id: event.document_id.clone(),
                path,
                user_id: event.user_id.clone(),
                source_kind: event.source_kind,
                state: event.state,
                seen_at: now,
            });
            status.editors.retain(|e| now - e.seen_at < EDITOR_TTL);
        });
    }
}
