//! Snapshots and checkpoints of a live edit session.
//!
//! A snapshot stores the session's engine state as a blob and prunes the
//! logged updates it covers. A checkpoint materialises the live model and
//! commits it as a revision with origin `edit_session`, based on the
//! session's previous checkpoint (or its base revision); when the revision
//! engine merges it with outside writes, the merged document is re-imported
//! with the live block ids carried over and written back into the live
//! model, logged and broadcast like any other update.
//!
//! A checkpoint always runs as the session's last editor. Settling a
//! session that is being left or closed lands a conflicting body as a
//! proposal on the checkpoint base, authored by that editor, so no live
//! edit is lost.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{ActorKind, DocumentAction, DocumentFormat};

use super::apply::update_event;
use super::document::{import_rich_text, materialise, unchanged_from};
use super::entry::{CollabEntry, model_error, utf8};
use super::model::body_of;
use super::{Checkpoint, CollabError, CollabService};
use crate::documents::engine::ModelView;
use crate::documents::model::{BlockKind, Media, RichText, carry_ids, media_refs};
use crate::documents::models::RevisionAttribution;
use crate::documents::repository::SnapshotMark;
use crate::documents::revision::CommitBody;
use crate::documents::service::{
    Author, CommentPolicy, CommitOutcome, CommitRequest, ProposeBodyRequest, RevisionOrigin,
    held_media_refs,
};

const CHECKPOINT_MESSAGE: &str = "Live edit session checkpoint";
const CONFLICT_MESSAGE: &str = "Live edit session body that conflicted with the head";

impl CollabService {
    pub(super) async fn snapshot(&self, entry: &mut CollabEntry) -> Result<(), CollabError> {
        self.catch_up(entry).await?;
        let bytes = entry.doc.snapshot()?;
        let version = entry.doc.version();
        let blob = self.documents.blobs().put(&bytes).await?;
        let moved = self
            .documents
            .repositories()
            .collab
            .set_snapshot(
                &entry.session.id,
                SnapshotMark {
                    hash: &blob.hash,
                    seq: entry.last_seq,
                    version: &version,
                },
            )
            .await?;
        if moved {
            entry.session.snapshot_sha256 = Some(blob.hash);
        }
        entry.unsnapshotted = 0;
        entry.snapshot_due_from = None;
        Ok(())
    }

    /// The live model with every image the editor may not place swapped
    /// for a placeholder paragraph, and the bytes of the images it may: an
    /// image is placed when `who` uploaded it or the base revision already
    /// holds it. A checkpoint never fails on an image; each one left out is
    /// logged.
    async fn admit_live(
        &self,
        format: DocumentFormat,
        who: &AuthenticatedUser,
        base_bytes: &[u8],
        model: ModelView,
    ) -> (ModelView, Media) {
        let mut rich = match model {
            ModelView::RichText(rich) => rich,
            other => return (other, Media::new()),
        };
        let held = held_media_refs(format, base_bytes);
        let admitted = self
            .documents
            .admit_media(self.uploads.as_deref(), &who.id, media_refs(&rich), &held)
            .await;
        for refused in &admitted.refused {
            tracing::warn!(
                %format,
                blob = %refused,
                "live checkpoint left out an image the editor may not place"
            );
        }
        placeholders(&mut rich, &admitted.refused);
        (ModelView::RichText(rich), admitted.media)
    }

    pub(super) async fn checkpoint(
        &self,
        entry: &mut CollabEntry,
        who: &AuthenticatedUser,
        trace: &TraceId,
    ) -> Result<Checkpoint, CollabError> {
        self.catch_up(entry).await?;
        let document_id = entry.session.document_id.clone();
        let Some(base) = entry.checkpoint_base().cloned() else {
            return Ok(Checkpoint::Unchanged);
        };
        let limit = self.documents.config().max_upload_bytes;
        let base_bytes = self.revision_bytes(&base, limit).await?;
        let exported = entry.doc.export_model();
        entry.checkpoint_due_from = None;
        let (model, media) = self.admit_live(entry.format, who, &base_bytes, exported).await;
        if unchanged_from(entry.format, &base_bytes, &model, &media)
            .map_err(model_error(&document_id))?
        {
            return Ok(Checkpoint::Unchanged);
        }
        let next = materialise(entry.format, &base_bytes, &model, &media)
            .map_err(model_error(&document_id))?;
        let outcome = self
            .documents
            .commit(
                who,
                CommitRequest {
                    document_id: document_id.clone(),
                    base_revision: Some(base),
                    body: CommitBody::Bytes(next),
                    author: session_author(entry, trace),
                    message: Some(CHECKPOINT_MESSAGE.to_owned()),
                    lock_holder_id: None,
                    comments: CommentPolicy::detach(),
                    op_id: None,
                    media: Media::new(),
                },
                trace,
            )
            .await?;
        let revision = match outcome {
            CommitOutcome::Committed(landed) => landed.revision,
            CommitOutcome::Merged(landed) => {
                let revision = landed.revision;
                let merged = self.revision_bytes(&revision.id, limit).await?;
                self.reconcile(entry, who, &merged, trace).await?;
                revision
            },
            CommitOutcome::Conflict { head, .. } => {
                tracing::warn!(
                    document_id = %document_id,
                    head_revision_id = %head.id,
                    "Edit session checkpoint conflicted with the head"
                );
                return Ok(Checkpoint::Conflict {
                    head: head.id,
                    proposal: None,
                });
            },
        };
        self.documents
            .repositories()
            .edit_sessions
            .set_checkpoint(&entry.session.id, &revision.id)
            .await?;
        entry.session.checkpoint_revision_id = Some(revision.id.clone());
        Ok(Checkpoint::Committed(revision.id))
    }

    pub(super) async fn settle(
        &self,
        entry: &mut CollabEntry,
        editor: &AuthenticatedUser,
        trace: &TraceId,
    ) -> Result<Checkpoint, CollabError> {
        let checkpoint = self.checkpoint(entry, editor, trace).await?;
        let Checkpoint::Conflict { head, .. } = checkpoint else {
            return Ok(checkpoint);
        };
        let Some(base) = entry.checkpoint_base().cloned() else {
            return Ok(Checkpoint::Conflict {
                head,
                proposal: None,
            });
        };
        let document_id = entry.session.document_id.clone();
        let limit = self.documents.config().max_upload_bytes;
        let base_bytes = self.revision_bytes(&base, limit).await?;
        let exported = entry.doc.export_model();
        let (model, media) = self
            .admit_live(entry.format, editor, &base_bytes, exported)
            .await;
        let body = materialise(entry.format, &base_bytes, &model, &media)
            .map_err(model_error(&document_id))?;
        let proposed = self
            .documents
            .propose_body(
                editor,
                ProposeBodyRequest {
                    document_id,
                    base_revision: base,
                    bytes: body,
                    author: session_author(entry, trace),
                    message: Some(CONFLICT_MESSAGE.to_owned()),
                },
                trace,
            )
            .await?;
        Ok(Checkpoint::Conflict {
            head,
            proposal: Some(proposed.proposal.id),
        })
    }

    async fn reconcile(
        &self,
        entry: &mut CollabEntry,
        who: &AuthenticatedUser,
        merged: &[u8],
        trace: &TraceId,
    ) -> Result<(), CollabError> {
        let document_id = entry.session.document_id.clone();
        let target = match entry.doc.export_model() {
            ModelView::RichText(live) => {
                let mut next =
                    import_rich_text(entry.format, merged).map_err(model_error(&document_id))?;
                carry_ids(&live, &mut next);
                ModelView::RichText(next)
            },
            ModelView::Body(_) => {
                ModelView::Body(body_of(entry.format, &utf8(&document_id, merged.to_vec())?))
            },
        };
        let Some(update) = entry.doc.update_to_model(&target)? else {
            return Ok(());
        };
        let seq = self.log_update(entry, &update, &who.id).await?;
        let document = self
            .authorized(who, &entry.session.document_id, DocumentAction::View, trace)
            .await?;
        self.broadcast(
            &document,
            update_event(&document, &entry.session.id, seq, &update, &who.id),
        )
        .await;
        Ok(())
    }
}

fn session_author(entry: &CollabEntry, trace: &TraceId) -> Author {
    Author {
        actor_kind: ActorKind::User,
        origin: RevisionOrigin::EditSession(entry.session.id.clone()),
        attribution: RevisionAttribution {
            trace_id: Some(trace.clone()),
            ..RevisionAttribution::default()
        },
    }
}

const PLACEHOLDER: &str = "[image not available]";

/// Replaces each image block whose blob is in `refused` with a paragraph
/// saying so, keeping its id, inside table cells too.
fn placeholders(model: &mut RichText, refused: &[String]) {
    if refused.is_empty() {
        return;
    }
    for block in &mut model.blocks {
        match &mut block.kind {
            BlockKind::Image { blob, .. } if refused.contains(blob) => {
                block.kind = BlockKind::Paragraph;
                block.text = PLACEHOLDER.to_owned();
                block.marks.clear();
            },
            BlockKind::Table { rows, .. } => {
                for cell in rows.iter_mut().flatten() {
                    placeholders(cell, refused);
                }
            },
            _ => {},
        }
    }
}
