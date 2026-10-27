//! Applying a client's Loro update: validate, log, import, broadcast.
//!
//! An update enters a replica's cache only from the log, after the
//! transaction that numbered it has committed, so a failed append leaves
//! the cache as it was.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use loro::{ExportMode, LoroDoc, VersionVector};
use systemprompt_identifiers::{DocumentEditSessionId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, DocumentCollabUpdateEvent};

use super::entry::CollabEntry;
use super::{Applied, ApplyRequest, CollabError, CollabService};
use crate::documents::models::Document;

impl CollabService {
    pub async fn apply(
        &self,
        who: &AuthenticatedUser,
        request: ApplyRequest,
        trace: &TraceId,
    ) -> Result<Applied, CollabError> {
        let document = self
            .authorized(who, &request.document_id, DocumentAction::Edit, trace)
            .await?;
        let repos = self.documents.repositories();
        if !repos.collab.is_editing(&document.id, &who.id).await? {
            return Err(CollabError::NotEditing {
                document_id: document.id,
            });
        }
        let session = self
            .open_matching(&document.id, &request.session_id)
            .await?;
        let client_version = request
            .version
            .as_deref()
            .map(VersionVector::decode)
            .transpose()
            .map_err(CollabError::InvalidVersion)?;
        let entry = self.entry(&document, &session).await?;
        let mut entry = entry.lock().await;
        self.catch_up(&mut entry).await?;
        let missing = match &client_version {
            Some(known) if !known.includes_vv(&entry.doc.oplog_vv()) => {
                Some(entry.doc.export(ExportMode::updates(known))?)
            },
            _ => None,
        };
        LoroDoc::decode_import_blob_meta(&request.update, true)
            .map_err(CollabError::InvalidUpdate)?;
        let seq = self
            .log_update(&mut entry, &request.update, &who.id)
            .await?;
        entry.record_activity(Some(who));
        let version = entry.doc.oplog_vv().encode();
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
