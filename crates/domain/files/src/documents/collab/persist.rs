//! Snapshots and checkpoints of a live edit session.
//!
//! A snapshot stores the session's Loro state as a blob and prunes the
//! logged updates it covers. A checkpoint materialises the live body and
//! commits it as a revision with origin `edit_session`, based on the
//! session's previous checkpoint (or its base revision); when the revision
//! engine merges it with outside writes, the merged body is written back
//! into the live text, logged and broadcast like any other update.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{ExportMode, UpdateOptions};
use systemprompt_identifiers::{DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{ActorKind, DocumentAction};

use super::entry::{CollabEntry, utf8};
use super::model::{BODY_CONTAINER, body_of, body_text, materialise};
use super::{CollabError, CollabService};
use crate::documents::revision::CommitBody;
use crate::documents::service::{Author, CommitOutcome, CommitRequest, RevisionOrigin};

const CHECKPOINT_MESSAGE: &str = "Live edit session checkpoint";

impl CollabService {
    pub(super) async fn snapshot(&self, entry: &mut CollabEntry) -> Result<(), CollabError> {
        self.catch_up(entry).await?;
        let bytes = entry.doc.export(ExportMode::Snapshot)?;
        let blob = self.documents.blobs().put(&bytes).await?;
        let moved = self
            .documents
            .repositories()
            .collab
            .set_snapshot(&entry.session.id, &blob.hash, entry.last_seq)
            .await?;
        if moved {
            entry.session.snapshot_sha256 = Some(blob.hash);
        }
        entry.unsnapshotted = 0;
        entry.snapshot_due_from = None;
        Ok(())
    }

    pub(super) async fn checkpoint(
        &self,
        entry: &mut CollabEntry,
        who: &AuthenticatedUser,
        trace: &TraceId,
    ) -> Result<Option<DocumentRevisionId>, CollabError> {
        self.catch_up(entry).await?;
        let document_id = entry.session.document_id.clone();
        let Some(base) = entry.checkpoint_base().cloned() else {
            return Ok(None);
        };
        let limit = self.documents.config().max_upload_bytes;
        let base_text = utf8(&document_id, self.revision_bytes(&base, limit).await?)?;
        let body = body_text(&entry.doc);
        let next = materialise(entry.format, &base_text, &body);
        entry.checkpoint_due_from = None;
        if next == base_text {
            return Ok(None);
        }
        let outcome = self
            .documents
            .commit(
                who,
                CommitRequest {
                    document_id: document_id.clone(),
                    base_revision: Some(base),
                    body: CommitBody::Bytes(next.into_bytes()),
                    author: Author {
                        actor_kind: ActorKind::User,
                        origin: RevisionOrigin::EditSession(entry.session.id.clone()),
                    },
                    message: Some(CHECKPOINT_MESSAGE.to_owned()),
                    lock_holder_id: None,
                },
                trace,
            )
            .await?;
        let revision = match outcome {
            CommitOutcome::Committed { revision, .. } => revision,
            CommitOutcome::Merged { revision, .. } => {
                let merged = self.revision_bytes(&revision.id, limit).await?;
                let merged = body_of(entry.format, &utf8(&document_id, merged)?);
                self.reconcile(entry, who, &merged, trace).await?;
                revision
            },
            CommitOutcome::Conflict { head, .. } => {
                tracing::warn!(
                    document_id = %document_id,
                    head_revision_id = %head.id,
                    "Edit session checkpoint conflicted with the head; retrying at the next checkpoint"
                );
                return Ok(None);
            },
        };
        self.documents
            .repositories()
            .edit_sessions
            .set_checkpoint(&entry.session.id, &revision.id)
            .await?;
        entry.session.checkpoint_revision_id = Some(revision.id.clone());
        Ok(Some(revision.id))
    }

    async fn reconcile(
        &self,
        entry: &mut CollabEntry,
        who: &AuthenticatedUser,
        merged_body: &str,
        trace: &TraceId,
    ) -> Result<(), CollabError> {
        if body_text(&entry.doc) == merged_body {
            return Ok(());
        }
        let before = entry.doc.oplog_vv();
        entry
            .doc
            .get_text(BODY_CONTAINER)
            .update(merged_body, UpdateOptions::default())
            .map_err(CollabError::Reconcile)?;
        entry.doc.commit();
        let update = entry.doc.export(ExportMode::updates(&before))?;
        let seq = self.log_update(entry, &update, &who.id).await?;
        let document = self
            .authorized(who, &entry.session.document_id, DocumentAction::View, trace)
            .await?;
        self.broadcast(&document, &entry.session.id, seq, &update, &who.id)
            .await;
        Ok(())
    }
}
