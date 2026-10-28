//! The Drive status the tray, the GUI and the CLI read, and the handle the
//! sync loop is driven through. Notices are numbered so a reader shows each
//! one once: it remembers the last `seq` it showed.
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
use super::shares::{ReceivedShare, received};
pub use super::state::ConflictNotice;
use super::state::{DriveState, OpenProposal, SentForReview};
use crate::proxy::drive_signal::DocumentNotice;

pub const EDITOR_TTL: Duration = Duration::seconds(90);
const NOTICES_KEPT: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueuedNotice {
    pub seq: u64,
    pub notice: DocumentNotice,
}

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
    pub proposals: Vec<OpenProposal>,
    pub sent_for_review: Vec<SentForReview>,
    pub shares: Vec<ReceivedShare>,
    pub notices: Vec<QueuedNotice>,
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
            proposals: state.proposals,
            sent_for_review: state.sent_for_review,
            shares: state.shares,
            editors: Vec::new(),
            notices: Vec::new(),
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
    shares: Mutex<Vec<ReceivedShare>>,
    generation: watch::Sender<u64>,
    pub(crate) cycle: tokio::sync::Mutex<()>,
}

impl Default for DriveHandle {
    fn default() -> Self {
        Self {
            status: ArcSwap::from_pointee(DriveStatus::default()),
            wake: Notify::new(),
            touched: Mutex::new(BTreeSet::new()),
            shares: Mutex::new(Vec::new()),
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

    #[must_use]
    pub fn take_shares(&self) -> Vec<ReceivedShare> {
        std::mem::take(&mut *self.shares.lock())
    }

    pub fn mark_enabled(&self, root: &DriveRoot) {
        self.update(|status| {
            status.enabled = true;
            status.root = Some(root.dir().to_path_buf());
        });
    }

    pub fn mark_disabled(&self) {
        self.update(|status| {
            status.enabled = false;
            status.last_error = None;
        });
    }

    pub fn push_notice(&self, notice: DocumentNotice) {
        if let Some(share) = received(&notice, Utc::now()) {
            self.shares.lock().push(share);
        }
        self.update(|status| {
            let seq = status.notices.last().map_or(1, |last| last.seq + 1);
            status.notices.push(QueuedNotice { seq, notice });
            let excess = status.notices.len().saturating_sub(NOTICES_KEPT);
            status.notices.drain(..excess);
        });
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
