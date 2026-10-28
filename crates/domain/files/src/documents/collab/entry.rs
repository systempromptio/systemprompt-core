//! The per-replica cache entry of an open edit session and how it is built
//! and caught up from the shared update log.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Instant;

use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentFormat;
use tokio::sync::Mutex;

use super::document::{ModelError, import};
use super::{CollabError, CollabService};
use crate::documents::engine::{EngineDoc, ModelView};
use crate::documents::error::DocumentsError;
use crate::documents::model::RichText;
use crate::documents::models::{Document, DocumentEditSession};

pub(super) type SharedEntry = Arc<Mutex<CollabEntry>>;

#[derive(Debug)]
pub(super) struct CollabEntry {
    pub(super) session: DocumentEditSession,
    pub(super) format: DocumentFormat,
    pub(super) doc: Box<dyn EngineDoc>,
    pub(super) last_seq: i64,
    pub(super) unsnapshotted: u32,
    pub(super) snapshot_due_from: Option<Instant>,
    pub(super) checkpoint_due_from: Option<Instant>,
    pub(super) last_editor: Option<AuthenticatedUser>,
    pub(super) touched: Instant,
}

impl CollabEntry {
    pub(super) fn checkpoint_base(&self) -> Option<&DocumentRevisionId> {
        self.session
            .checkpoint_revision_id
            .as_ref()
            .or(self.session.base_revision_id.as_ref())
    }

    pub(super) fn record_activity(&mut self, editor: Option<&AuthenticatedUser>) {
        let now = Instant::now();
        self.unsnapshotted = self.unsnapshotted.saturating_add(1);
        self.snapshot_due_from.get_or_insert(now);
        self.checkpoint_due_from.get_or_insert(now);
        if let Some(editor) = editor {
            self.last_editor = Some(editor.clone());
        }
        self.touched = now;
    }
}

impl CollabService {
    pub(super) async fn entry(
        &self,
        document: &Document,
        session: &DocumentEditSession,
    ) -> Result<SharedEntry, CollabError> {
        if let Some(entry) = self.cached(&document.id) {
            if entry.lock().await.session.id == session.id {
                return Ok(entry);
            }
            self.cache_remove(&document.id);
        }
        let doc = self.load_doc(document, session).await?;
        let built = Arc::new(Mutex::new(CollabEntry {
            session: session.clone(),
            format: document.format,
            doc,
            last_seq: 0,
            unsnapshotted: 0,
            snapshot_due_from: None,
            checkpoint_due_from: None,
            last_editor: None,
            touched: Instant::now(),
        }));
        Ok(self.cache_insert(document.id.clone(), built))
    }

    async fn load_doc(
        &self,
        document: &Document,
        session: &DocumentEditSession,
    ) -> Result<Box<dyn EngineDoc>, CollabError> {
        let engine = self.engine()?;
        let limit = self.documents.config().max_upload_bytes;
        if let Some(hash) = &session.snapshot_sha256 {
            let bytes = self.documents.blobs().read_all(hash, u64::MAX).await?;
            return Ok(engine.open(&bytes)?);
        }
        let model = match &session.base_revision_id {
            Some(revision) => {
                let bytes = self.revision_bytes(revision, limit).await?;
                import(document.format, &bytes).map_err(model_error(&document.id))?
            },
            None => ModelView::RichText(RichText::default()),
        };
        Ok(engine.seed(&model)?)
    }

    pub(super) async fn revision_bytes(
        &self,
        revision: &DocumentRevisionId,
        limit: u64,
    ) -> Result<Vec<u8>, CollabError> {
        let found = self
            .documents
            .repositories()
            .revisions
            .find(revision)
            .await?
            .ok_or_else(|| DocumentsError::NotFound {
                what: "revision",
                id: revision.to_string(),
            })?;
        Ok(self
            .documents
            .blobs()
            .read_all(&found.content_sha256, limit)
            .await?)
    }

    pub(super) async fn catch_up(&self, entry: &mut CollabEntry) -> Result<(), CollabError> {
        let rows = self
            .documents
            .repositories()
            .collab
            .list_after(&entry.session.id, entry.last_seq)
            .await?;
        for row in rows {
            entry.doc.apply_update(&row.update)?;
            entry.last_seq = entry.last_seq.max(row.seq);
        }
        Ok(())
    }
}

pub(super) fn model_error(document: &DocumentId) -> impl FnOnce(ModelError) -> CollabError {
    let document_id = document.clone();
    move |source| CollabError::Model {
        document_id,
        source,
    }
}

pub(super) fn utf8(document: &DocumentId, bytes: Vec<u8>) -> Result<String, CollabError> {
    String::from_utf8(bytes).map_err(|source| CollabError::NotText {
        document_id: document.clone(),
        source,
    })
}
