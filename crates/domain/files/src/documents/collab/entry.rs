//! The per-replica cache entry of an open edit session and how it is built
//! and caught up from the shared update log.
//!
//! An entry is always opened from the session's stored snapshot. A session
//! that has none yet is seeded from its base revision once, by whichever
//! replica records the seed snapshot first, and every replica then opens that
//! stored seed, so all of them hold the same engine operations. Catching up
//! reopens the stored snapshot when another replica's snapshot has pruned
//! updates this entry has not imported, then imports the updates after it,
//! checking each logged update's header before the engine sees it and
//! folding each update's recorded actor into the checkpoint actor. The model
//! is exported once before and once after a batch; only a batch that adds an
//! image reference and holds updates of more than one author is replayed on
//! a fork, one export per run of one author, so each new reference is
//! attributed to the editor whose update added it.
//!
//! The backlog read and any snapshot blob it reopens run without the entry
//! held; the entry is then taken, and the fetched updates apply only when no
//! other caller caught it up meanwhile (otherwise the read is retried), so a
//! slow read never blocks the updates applied to the same session.
//!
//! A session whose snapshot predates the engine's current container shape
//! (a table that keeps its cells in its `kind`) is upgraded when it is
//! joined: the engine's update from the session to its own model is logged
//! under the `collab-write-back` system actor and broadcast, so it is
//! nobody's authorship and every replica converges on the new shape.
//!
//! A session whose snapshot or log the engine cannot read is closed rather
//! than left failing every call: the next join opens a new session seeded
//! from the head revision, so the document stays readable and editable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Instant;

use systemprompt_identifiers::{ContentHash, DocumentRevisionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentFormat;
use tokio::sync::Mutex;

use super::apply::update_event;
use super::document::import;
use super::error::model_error;
use super::image_owners::ImageOwners;
use super::lease::SessionKey;
use super::reconcile::WRITE_BACK_ACTOR;
use super::{CollabError, CollabService};
use crate::documents::agents::DocumentActor;
use crate::documents::engine::{EngineDoc, EngineError, ModelView};
use crate::documents::error::DocumentsError;
use crate::documents::model::{EditLevel, RichText};
use crate::documents::models::{Document, DocumentEditSession};
use crate::documents::repository::UpdateAuthor;

pub(super) type SharedEntry = Arc<Mutex<CollabEntry>>;

#[derive(Debug)]
pub(super) struct CollabEntry {
    pub(super) session: DocumentEditSession,
    pub(super) format: DocumentFormat,
    pub(super) doc: Box<dyn EngineDoc>,
    pub(super) last_seq: i64,
    pub(super) unsnapshotted: u32,
    pub(super) snapshot_due_from: Option<Instant>,
    pub(super) snapshotting: bool,
    pub(super) checkpoint_due_from: Option<Instant>,
    pub(super) last_editor: Option<AuthenticatedUser>,
    pub(super) last_actor: Option<DocumentActor>,
    pub(super) last_joiner: Option<AuthenticatedUser>,
    pub(super) touched: Instant,
    pub(super) images: ImageOwners,
    pub(super) checkpoint_failures: u32,
    pub(super) checkpoint_retry_at: Option<Instant>,
}

impl CollabEntry {
    pub(super) fn checkpoint_base(&self) -> Option<&DocumentRevisionId> {
        self.session
            .checkpoint_revision_id
            .as_ref()
            .or(self.session.base_revision_id.as_ref())
    }

    pub(super) fn record_activity(&mut self, editor: &AuthenticatedUser) {
        let now = Instant::now();
        self.unsnapshotted = self.unsnapshotted.saturating_add(1);
        self.snapshot_due_from.get_or_insert(now);
        self.checkpoint_due_from.get_or_insert(now);
        self.last_editor = Some(editor.clone());
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
        let loaded = self.load_doc(document, session).await;
        let (doc, images, last_seq) = match loaded {
            Err(error) if is_unreadable(&error) => {
                return Err(self.quarantine(session, &error).await);
            },
            other => other?,
        };
        let built = Arc::new(Mutex::new(CollabEntry {
            session: session.clone(),
            format: document.format,
            doc,
            last_seq,
            unsnapshotted: 0,
            snapshot_due_from: None,
            snapshotting: false,
            checkpoint_due_from: None,
            last_editor: None,
            last_actor: None,
            last_joiner: None,
            touched: Instant::now(),
            images,
            checkpoint_failures: 0,
            checkpoint_retry_at: None,
        }));
        Ok(self.cache_insert(document.id.clone(), built))
    }

    pub(super) async fn upgrade_shape(
        &self,
        shared: &SharedEntry,
        document: &Document,
        who: &AuthenticatedUser,
    ) -> Result<(), CollabError> {
        let (update, key) = {
            let entry = self.caught_up(shared).await?;
            let own = entry.doc.export_model()?;
            (entry.doc.update_to_model(&own)?, SessionKey::of(&entry))
        };
        let Some(update) = update else {
            return Ok(());
        };
        let actor = DocumentActor::system(WRITE_BACK_ACTOR);
        let author = UpdateAuthor {
            user: &who.id,
            actor: &actor,
            level: EditLevel::Edit,
        };
        let seq = self.log_update(shared, &key, &update, author).await?;
        self.broadcast(
            document,
            update_event(document, &key.session, seq, &update, &who.id),
        )
        .await;
        Ok(())
    }

    async fn adopt_projection_ids(
        &self,
        document: &Document,
        revision: &DocumentRevisionId,
        seeded: &mut dyn EngineDoc,
    ) {
        let adopted = match self.documents.projection_with_ids(document, revision).await {
            Ok(projection) => seeded
                .adopt_node_ids(&projection.root)
                .map_err(CollabError::from),
            Err(error) => Err(error.into()),
        };
        if let Err(error) = adopted {
            tracing::warn!(
                document_id = %document.id,
                error = %error,
                "Live session seeded without the projection's node ids"
            );
        }
    }

    async fn load_doc(
        &self,
        document: &Document,
        session: &DocumentEditSession,
    ) -> Result<(Box<dyn EngineDoc>, ImageOwners, i64), CollabError> {
        let collab = &self.documents.repositories().collab;
        if let Some(stored) = collab.stored_snapshot(&session.id).await? {
            let (doc, owners) = self.open_snapshot(&stored.hash).await?;
            return Ok((doc, owners, stored.seq));
        }
        let engine = self.engine()?;
        let limit = self.documents.config().max_upload_bytes;
        let model = match &session.base_revision_id {
            Some(revision) => {
                let bytes = self.revision_bytes(revision, limit).await?;
                import(document.format, &bytes).map_err(model_error(&document.id))?
            },
            None => ModelView::RichText(RichText::default()),
        };
        let mut seeded = engine.seed(&model)?;
        if let Some(revision) = &session.base_revision_id {
            self.adopt_projection_ids(document, revision, seeded.as_mut())
                .await;
        }
        let sealed = ImageOwners::default().seal(&seeded.snapshot()?);
        let blob = self.documents.blobs().put(&sealed).await?;
        collab
            .seed_snapshot(&session.id, &blob.hash, &seeded.version())
            .await?;
        let stored = collab.stored_snapshot(&session.id).await?.ok_or_else(|| {
            CollabError::SessionClosed {
                document_id: document.id.clone(),
                session_id: session.id.clone(),
            }
        })?;
        let (doc, owners) = self.open_snapshot(&stored.hash).await?;
        Ok((doc, owners, stored.seq))
    }

    pub(super) async fn open_snapshot(
        &self,
        hash: &ContentHash,
    ) -> Result<(Box<dyn EngineDoc>, ImageOwners), CollabError> {
        let engine = self.engine()?;
        let limit = self.documents.config().max_upload_bytes.saturating_mul(4);
        let bytes = self.documents.blobs().read_all(hash, limit).await?;
        let (owners, snapshot) = ImageOwners::open(&bytes)?;
        Ok((engine.open(snapshot)?, owners))
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

    pub(super) async fn quarantine(
        &self,
        session: &DocumentEditSession,
        error: &CollabError,
    ) -> CollabError {
        tracing::error!(
            document_id = %session.document_id,
            session_id = %session.id,
            error = %error,
            "Live session state is unreadable; closing the session so the next join reseeds it \
             from the head revision"
        );
        if let Err(close) = self
            .documents
            .repositories()
            .edit_sessions
            .close(&session.id)
            .await
        {
            tracing::warn!(
                session_id = %session.id,
                error = %close,
                "Unreadable live session could not be closed"
            );
        }
        self.cache_remove(&session.document_id);
        CollabError::SessionClosed {
            document_id: session.document_id.clone(),
            session_id: session.id.clone(),
        }
    }
}

pub(super) const fn is_unreadable(error: &CollabError) -> bool {
    matches!(
        error,
        CollabError::Engine(
            EngineError::Model(_) | EngineError::State(_) | EngineError::InvalidUpdate(_)
        )
    )
}
