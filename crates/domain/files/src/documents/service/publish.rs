//! What follows a new head revision: the search index, the
//! `document.revision.created` event, published on the document's live
//! topic only, and the `document.drive.changed` hint to the document's
//! access holders (see `drive_hint`). None can undo the revision; the
//! index and the event report failure as `false` after logging it, and the
//! hint runs in the background.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::documents::DocumentRevisionCreatedEvent;

use super::DocumentService;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision};

impl DocumentService {
    pub(super) async fn index_text(
        &self,
        document: &Document,
        revision: &DocumentRevision,
        bytes: Vec<u8>,
    ) -> bool {
        let indexed = match self.plain_text_of(document.format, bytes).await {
            Ok(text) => self
                .repos
                .text_index
                .upsert(&document.id, &revision.id, &text)
                .await
                .map_err(DocumentsError::from),
            Err(error) => Err(error),
        };
        if let Err(error) = indexed {
            tracing::error!(
                document_id = %document.id,
                revision_id = %revision.id,
                error = %error,
                "Search index not updated for the new head revision"
            );
            return false;
        }
        true
    }

    pub(super) async fn announce_revision(
        &self,
        document: &Document,
        revision: &DocumentRevision,
    ) -> bool {
        let event = DocumentRevisionCreatedEvent {
            document_id: document.id.clone(),
            revision_id: revision.id.clone(),
            parent_revision_id: revision.parent_revision_id.clone(),
            content_sha256: revision.content_sha256.clone(),
            author_id: revision.author_user_id.clone(),
            actor_kind: revision.actor_kind,
            origin_kind: revision.origin.kind(),
        };
        self.hint_drive_changed(document, &revision.author_user_id);
        self.events.revision_created(&event).await.is_ok()
    }
}
