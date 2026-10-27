//! One repository per document store table, built once into
//! [`DocumentRepositories`] at the composition root.
//!
//! Visibility is decided in SQL: a listing or search returns a document when
//! the caller owns it, or when an unexpired grant on the document or on any
//! folder in its ancestor chain names one of the caller's subject pairs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod blobs;
mod documents;
mod edit_sessions;

mod folder_contents;
mod folders;
mod grants;
mod locks;
mod presence;
mod purge;
mod revisions;
mod rows;
mod text_index;

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;

pub use blobs::DocumentBlobRepository;
pub use documents::DocumentRepository;
pub use edit_sessions::DocumentEditSessionRepository;
pub use folders::DocumentFolderRepository;
pub use grants::DocumentGrantRepository;
pub use locks::DocumentLockRepository;
pub use presence::DocumentPresenceRepository;
pub use revisions::DocumentRevisionRepository;
pub use text_index::DocumentTextIndexRepository;

#[derive(Debug, Clone)]
pub struct DocumentRepositories {
    pub folders: Arc<DocumentFolderRepository>,
    pub documents: Arc<DocumentRepository>,
    pub revisions: Arc<DocumentRevisionRepository>,
    pub blobs: Arc<DocumentBlobRepository>,
    pub grants: Arc<DocumentGrantRepository>,
    pub locks: Arc<DocumentLockRepository>,
    pub presence: Arc<DocumentPresenceRepository>,
    pub edit_sessions: Arc<DocumentEditSessionRepository>,
    pub text_index: Arc<DocumentTextIndexRepository>,
}

impl DocumentRepositories {
    pub fn new(db: &DbPool) -> Self {
        Self {
            folders: Arc::new(DocumentFolderRepository::new(db)),
            documents: Arc::new(DocumentRepository::new(db)),
            revisions: Arc::new(DocumentRevisionRepository::new(db)),
            blobs: Arc::new(DocumentBlobRepository::new(db)),
            grants: Arc::new(DocumentGrantRepository::new(db)),
            locks: Arc::new(DocumentLockRepository::new(db)),
            presence: Arc::new(DocumentPresenceRepository::new(db)),
            edit_sessions: Arc::new(DocumentEditSessionRepository::new(db)),
            text_index: Arc::new(DocumentTextIndexRepository::new(db)),
        }
    }
}

#[derive(Debug, Clone)]
struct Pools {
    read: Arc<PgPool>,
    write: Arc<PgPool>,
}

impl Pools {
    fn new(db: &DbPool) -> Self {
        Self {
            read: db.pool(),
            write: db.write_pool(),
        }
    }
}
