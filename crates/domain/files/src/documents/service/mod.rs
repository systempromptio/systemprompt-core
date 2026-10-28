//! [`DocumentService`]: folders, documents, the revision engine, locks,
//! presence, sharing, access requests, search and the bridge Drive feed.
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
//! An edit can instead be proposed: written as a revision off the head
//! chain and landed only when a reviewer accepts it (see `proposals` and
//! `proposal_review`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod access_requests;
mod access_view;
mod acks;
mod apply;
mod byte_diff;
mod comments;
mod comments_edit;
mod comments_export;
mod commit;
mod commit_build;
mod documents;
mod drive;
mod drive_blob;
mod drive_commit;
mod drive_feed;
mod first_revision;
mod folders;
mod fragments;
mod grants;
mod landed;
mod landing;
mod lifecycle;
mod limits;
mod listing;
mod locks;
mod node_ids;
mod ownership;
mod pdf;
mod prepare;
mod proposal_body;
mod proposal_reads;
mod proposal_review;
mod proposals;
mod publish;
mod reads;
mod support;
mod types;

use std::sync::Arc;

pub use access_requests::{AccessDecision, AccessRequestInput, DecideRequest};
pub use access_view::{
    DocumentAccess, FolderAccess, InheritedGrant, MAX_GRANT_HISTORY, TargetAccess,
};
pub use comments::{CommentRequest, ResolveCommentRequest};
pub use comments_edit::{DeleteCommentRequest, EditCommentRequest};
pub use comments_export::ExportedDocument;
pub use drive::parse_drive_cursor;
pub use drive_feed::CHANGE_LAG;
pub use grants::{MAX_SHARE_MESSAGE_BYTES, ShareRequest};
pub use lifecycle::{CopyRequest, FolderMove, UpdateRequest};
pub use ownership::TransferRequest;
pub use pdf::RenderedPdf;
pub use proposal_body::ProposeBodyRequest;
pub use proposal_reads::ProposalView;
pub use proposal_review::{Accepted, ReviewRequest};
pub use proposals::{EditModeRequest, ProposeRequest, Proposed};
pub use types::{
    AcquireLockRequest, Author, CommentPolicy, CommitOutcome, CommitRequest, CommitWarning,
    CreateRequest, DocumentView, HistoryPage, ImportRequest, LandedCommit, ListFolderRequest,
    Listing, NewFolderRequest, RevisionOrigin, SearchResult, StreamRequest,
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
    #[cfg(feature = "collab")]
    collab: Arc<std::sync::OnceLock<std::sync::Weak<super::collab::CollabService>>>,
}

/// The collaborators a [`DocumentService`] is built from.
#[derive(Debug, Clone)]
pub struct DocumentServiceParts {
    pub repos: Arc<DocumentRepositories>,
    pub blobs: Arc<BlobStore>,
    pub codecs: Arc<CodecRegistry>,
    pub templates: Arc<TemplateRegistry>,
    pub access: Arc<DocumentAccessService>,
    pub events: Arc<DocumentEvents>,
    pub config: DocumentsConfig,
}

impl DocumentService {
    #[must_use]
    pub fn new(parts: DocumentServiceParts) -> Self {
        Self {
            repos: parts.repos,
            blobs: parts.blobs,
            codecs: parts.codecs,
            templates: parts.templates,
            access: parts.access,
            events: parts.events,
            config: parts.config,
            #[cfg(feature = "collab")]
            collab: Arc::default(),
        }
    }

    #[cfg(feature = "collab")]
    pub fn attach_collab(&self, collab: &Arc<super::collab::CollabService>) {
        if self.collab.set(Arc::downgrade(collab)).is_err() {
            tracing::warn!("A live co-editing service is already attached to the document service");
        }
    }

    #[cfg(feature = "collab")]
    pub(super) fn collab(&self) -> Option<Arc<super::collab::CollabService>> {
        self.collab.get().and_then(std::sync::Weak::upgrade)
    }

    #[must_use]
    pub fn access(&self) -> &DocumentAccessService {
        &self.access
    }

    #[must_use]
    pub const fn repositories(&self) -> &Arc<DocumentRepositories> {
        &self.repos
    }

    #[must_use]
    pub const fn blobs(&self) -> &Arc<BlobStore> {
        &self.blobs
    }

    #[must_use]
    pub const fn events(&self) -> &Arc<DocumentEvents> {
        &self.events
    }

    #[must_use]
    pub const fn config(&self) -> &DocumentsConfig {
        &self.config
    }
}
