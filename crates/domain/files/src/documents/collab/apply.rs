//! Applying a client's engine update: validate, log, import, broadcast.
//!
//! An update is first applied to a fork of the replica's live document; the
//! fork's model is checked against the model schema, the limits and the
//! caller's access level ([`validate`]), and only an accepted update is
//! logged. An update enters a replica's cache only from the log, after the
//! transaction that numbered it has committed, so a failed append leaves
//! the cache as it was.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use systemprompt_identifiers::{DocumentEditSessionId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DocumentAction, DocumentCollabUpdateEvent};

use super::entry::CollabEntry;
use super::{Applied, ApplyRequest, CollabError, CollabService};
use crate::documents::engine::{EngineDoc, ModelView};
use crate::documents::model::{EditLevel, Limits, ModelRejection, RichText, validate};
use crate::documents::models::Document;

impl CollabService {
    pub async fn apply(
        &self,
        who: &AuthenticatedUser,
        request: ApplyRequest,
        trace: &TraceId,
    ) -> Result<Applied, CollabError> {
        let (document, level) = self
            .granted(who, &request.document_id, DocumentAction::Comment, trace)
            .await?;
        let level = if level >= AccessLevel::Edit {
            EditLevel::Edit
        } else {
            EditLevel::Comment
        };
        let repos = self.documents.repositories();
        if !repos.collab.is_editing(&document.id, &who.id).await? {
            return Err(CollabError::NotEditing {
                document_id: document.id,
            });
        }
        let session = self
            .open_matching(&document.id, &request.session_id)
            .await?;
        let engine = self.engine()?;
        let entry = self.entry(&document, &session).await?;
        let mut entry = entry.lock().await;
        self.catch_up(&mut entry).await?;
        let missing = request
            .version
            .as_deref()
            .map(|known| entry.doc.missing_since(known))
            .transpose()
            .map_err(CollabError::InvalidVersion)?
            .flatten();
        engine
            .check_update(&request.update)
            .map_err(CollabError::InvalidUpdate)?;
        let models = check_on_fork(entry.doc.as_ref(), &request.update, level)?;
        let seq = self
            .log_update(&mut entry, &request.update, &who.id)
            .await?;
        entry.record_activity(Some(who));
        if let Some((before, after)) = &models {
            entry.images.record(before, after, &who.id);
        }
        let version = entry.doc.version();
        if entry.unsnapshotted >= self.tuning.snapshot_after_updates {
            self.snapshot(&mut entry).await?;
        }
        drop(entry);
        self.broadcast(
            &document,
            update_event(&document, &session.id, seq, &request.update, &who.id),
        )
        .await;
        Ok(Applied {
            seq,
            version,
            missing,
        })
    }

    pub(super) async fn log_update(
        &self,
        entry: &mut CollabEntry,
        update: &[u8],
        author: &UserId,
    ) -> Result<i64, CollabError> {
        let seq = self
            .documents
            .repositories()
            .collab
            .append(&entry.session.id, update, author)
            .await?
            .ok_or_else(|| CollabError::SessionClosed {
                document_id: entry.session.document_id.clone(),
                session_id: entry.session.id.clone(),
            })?;
        self.catch_up(entry).await?;
        Ok(seq)
    }

    pub(super) async fn broadcast(&self, document: &Document, event: DocumentCollabUpdateEvent) {
        let to = self.recipients(document).await;
        if let Err(error) = self.documents.events().collab_update(&to, &event).await {
            tracing::warn!(
                document_id = %document.id,
                seq = event.seq,
                error = %error,
                "Collab update event not delivered"
            );
        }
    }
}

/// Checks the update on a fork; a rich-text update answers the models
/// before and after it, so the caller can record the images it placed.
fn check_on_fork(
    doc: &dyn EngineDoc,
    update: &[u8],
    level: EditLevel,
) -> Result<Option<(RichText, RichText)>, CollabError> {
    let mut draft = doc.fork();
    draft
        .apply_update(update)
        .map_err(CollabError::InvalidUpdate)?;
    let before = doc.export_model();
    match draft.export_model() {
        ModelView::RichText(after) => {
            let before = match before {
                ModelView::RichText(before) => before,
                ModelView::Body(_) => RichText::default(),
            };
            validate(&before, &after, level, &Limits::default())
                .map_err(CollabError::UpdateRejected)?;
            Ok(Some((before, after)))
        },
        after if level == EditLevel::Comment && after != before => Err(
            CollabError::UpdateRejected(ModelRejection::BeyondCommentLevel),
        ),
        ModelView::Body(_) => Ok(None),
    }
}

pub(super) fn update_event(
    document: &Document,
    session: &DocumentEditSessionId,
    seq: i64,
    update: &[u8],
    author: &UserId,
) -> DocumentCollabUpdateEvent {
    DocumentCollabUpdateEvent {
        document_id: document.id.clone(),
        edit_session_id: session.clone(),
        seq,
        update_b64: STANDARD.encode(update),
        author_id: author.clone(),
    }
}
