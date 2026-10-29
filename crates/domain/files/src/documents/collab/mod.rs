//! Live co-editing of HTML, Markdown and (with `office`) `docx` and `odt`
//! documents over a CRDT engine.
//!
//! [`CollabService`] keeps at most one open edit session per document
//! (`document_edit_sessions`) and, on each replica, one cached engine document
//! per session. A cached document is rebuilt from the session's snapshot blob,
//! or seeded from its base revision as a `RichText` model when it has none
//! (see [`document`]), and
//! then every logged update after it is imported. Before each read or write
//! a replica imports the updates other replicas logged since it last
//! looked, so every replica converges on the same log.
//!
//! Clients join (`edit` needs edit access, `comment` comment access, `view`
//! read access) and learn the engine, wire version, model and access level
//! they are bound to; they post engine updates, each applied to a fork and
//! validated before it is logged, and receive everyone's updates as
//! `document.collab.update` events on their own AG-UI stream; the event router
//! relays them to other replicas. Each update is logged with a sequence number
//! before it is broadcast.
//!
//! One background task per replica, started by [`CollabService::start`],
//! snapshots a session to a blob 30 s after its first unsnapshotted update
//! or after 200 updates, pruning the updates the snapshot covers, and
//! commits the live model as a revision with origin `edit_session` after
//! 2 min of activity; the last editor leaving commits and closes the
//! session. When a checkpoint merges with writes made outside the session,
//! the merged document is written back into the live model so the next
//! checkpoint keeps them. Spreadsheets and decks are refused with
//! [`DocumentsError::UnsupportedFormat`](super::DocumentsError).
//!
//! The CRDT itself is a [`DocumentEngine`] the composition root passes in;
//! with none, every session call answers [`CollabError::EngineUnavailable`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod access;
mod apply;
pub mod document;
mod entry;
mod error;
mod image_owners;
mod live_media;
pub mod model;
mod persist;
mod session;
mod timer;
mod trash;
mod types;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use systemprompt_identifiers::DocumentId;

pub use error::CollabError;
pub use image_owners::ImageOwners;
pub use types::{
    Applied, ApplyRequest, Checkpoint, CollabTuning, CursorRequest, JoinMode, Joined, Left,
};

use self::entry::SharedEntry;
use super::engine::DocumentEngine;
use super::service::DocumentService;
use crate::uploads::UploadService;

/// The live co-editing service of one replica, built once at a composition
/// root; its cache is per replica and its log is shared.
#[derive(Debug)]
pub struct CollabService {
    documents: Arc<DocumentService>,
    tuning: CollabTuning,
    engine: Option<Arc<dyn DocumentEngine>>,
    uploads: Option<Arc<UploadService>>,
    cache: Mutex<HashMap<DocumentId, SharedEntry>>,
}

impl CollabService {
    #[must_use]
    pub fn new(
        documents: Arc<DocumentService>,
        tuning: CollabTuning,
        engine: Option<Arc<dyn DocumentEngine>>,
    ) -> Self {
        Self {
            documents,
            tuning,
            engine,
            uploads: None,
            cache: Mutex::new(HashMap::new()),
        }
    }

    #[must_use]
    pub fn with_uploads(mut self, uploads: Arc<UploadService>) -> Self {
        self.uploads = Some(uploads);
        self
    }

    fn engine(&self) -> Result<&Arc<dyn DocumentEngine>, CollabError> {
        self.engine.as_ref().ok_or(CollabError::EngineUnavailable)
    }

    fn cached(&self, document: &DocumentId) -> Option<SharedEntry> {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(document)
            .cloned()
    }

    fn cache_insert(&self, document: DocumentId, entry: SharedEntry) -> SharedEntry {
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(cache.entry(document).or_insert(entry))
    }

    fn cache_remove(&self, document: &DocumentId) {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(document);
    }

    fn cached_entries(&self) -> Vec<(DocumentId, SharedEntry)> {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|(id, entry)| (id.clone(), Arc::clone(entry)))
            .collect()
    }
}
