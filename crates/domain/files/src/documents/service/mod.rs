//! [`DocumentService`]: folders, documents, the revision engine, locks,
//! presence, sharing, search and the bridge Drive feed.
//!
//! Every method authorizes through [`DocumentAccessService`] before it reads
//! or writes, and returns [`DocumentsError`](super::error::DocumentsError).
//! Codec work is CPU-bound and runs on the blocking pool.
//!
//! A commit lands by compare-and-swap on the document's head: a revision and
//! blob are written first, then the head moves only if it is still the
//! revision the commit started from. When a concurrent commit wins, the
//! loser gets [`CommitOutcome::Conflict`] and its revision row stays in the
//! document's history as a branch off the old head, keeping its blob
//! referenced; nothing is rolled back and no head ever points at it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod commit;
mod documents;
mod drive;
mod drive_commit;
mod folders;
mod grants;
mod listing;
mod locks;
mod publish;
mod reads;
mod support;
mod types;

use std::sync::Arc;

pub use drive::{CHANGE_LAG, parse_drive_cursor};
pub use grants::ShareRequest;
pub use types::{
    Author, CommitOutcome, CommitRequest, CommitWarning, CreateRequest, DocumentView,
    ImportRequest, Listing, NewFolderRequest, RevisionOrigin, SearchResult,
};

use super::access::DocumentAccessService;
use super::blob_store::BlobStore;
use super::codecs::CodecRegistry;
use super::events::DocumentEvents;
use super::repository::DocumentRepositories;
use super::templates::TemplateRegistry;
use crate::config::DocumentsConfig;

/// The document store's application service, built once at a composition
/// root and shared.
#[derive(Debug, Clone)]
pub struct DocumentService {
    repos: Arc<DocumentRepositories>,
    blobs: Arc<BlobStore>,
    codecs: Arc<CodecRegistry>,
    templates: Arc<TemplateRegistry>,
    access: Arc<DocumentAccessService>,
    events: Arc<DocumentEvents>,
    config: DocumentsConfig,
}

impl DocumentService {
    pub const fn new(
        repos: Arc<DocumentRepositories>,
        blobs: Arc<BlobStore>,
        codecs: Arc<CodecRegistry>,
        templates: Arc<TemplateRegistry>,
        access: Arc<DocumentAccessService>,
        events: Arc<DocumentEvents>,
        config: DocumentsConfig,
    ) -> Self {
        Self {
            repos,
            blobs,
            codecs,
            templates,
            access,
            events,
            config,
        }
    }

    #[must_use]
    pub fn access(&self) -> &DocumentAccessService {
        &self.access
    }

    #[must_use]
    pub const fn config(&self) -> &DocumentsConfig {
        &self.config
    }
}
