//! The comment edit, delete and mention events, emitted through
//! [`DocumentEvents`] like every other document event: routed to each
//! recipient and published once on the document's live topic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;
use systemprompt_models::documents::{
    DocumentCommentDeletedEvent, DocumentCommentMentionedEvent, DocumentCommentUpdatedEvent,
};

use super::error::DocumentsError;
use super::events::DocumentEvents;

impl DocumentEvents {
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
        self.emit(DocumentCommentMentionedEvent::NAME, to, event)
            .await
    }
}
