//! The comment events, emitted through [`DocumentEvents`]: routed to each
//! recipient, and creation, resolution, edits and deletions also published
//! once on the document's live topic; the mention reaches its recipient
//! only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;
use systemprompt_models::documents::{
    DocumentCommentCreatedEvent, DocumentCommentDeletedEvent, DocumentCommentMentionedEvent,
    DocumentCommentResolvedEvent, DocumentCommentUpdatedEvent,
};

use super::error::DocumentsError;
use super::events::{Audience, DocumentEvents};

impl DocumentEvents {
    pub async fn comment_created(
        &self,
        to: &[UserId],
        event: &DocumentCommentCreatedEvent,
    ) -> Result<(), DocumentsError> {
        self.emit(DocumentCommentCreatedEvent::NAME, to, event)
            .await
    }

    pub async fn comment_resolved(
        &self,
        to: &[UserId],
        event: &DocumentCommentResolvedEvent,
    ) -> Result<(), DocumentsError> {
        self.emit(DocumentCommentResolvedEvent::NAME, to, event)
            .await
    }

    pub async fn comment_updated(
        &self,
        to: &[UserId],
        event: &DocumentCommentUpdatedEvent,
    ) -> Result<(), DocumentsError> {
        self.emit(DocumentCommentUpdatedEvent::NAME, to, event)
            .await
    }

    pub async fn comment_deleted(
        &self,
        to: &[UserId],
        event: &DocumentCommentDeletedEvent,
    ) -> Result<(), DocumentsError> {
        self.emit(DocumentCommentDeletedEvent::NAME, to, event)
            .await
    }

    pub async fn comment_mentioned(
        &self,
        to: &[UserId],
        event: &DocumentCommentMentionedEvent,
    ) -> Result<(), DocumentsError> {
        self.emit_to(
            DocumentCommentMentionedEvent::NAME,
            to,
            event,
            Audience::Recipients,
        )
        .await
    }
}
