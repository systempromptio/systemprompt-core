//! Reopening a resolved comment: its own action, not an undo of the
//! resolution. It clears the resolution and records who reopened it and when.
//!
//! It needs the same `comment` access as resolving. The reopening and the
//! durable `document.comment.reopened` brain fact commit in one transaction
//! on the primary, so a sink failure leaves the comment resolved; the live
//! `DocumentEvents` frame goes after commit to the audience resolution
//! reaches. Reopening a comment that is already open answers the comment as
//! it stands and records and sends nothing, as resolving a resolved comment
//! keeps its first resolution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentCommentId, DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, DocumentCommentReopenedEvent};
use systemprompt_traits::RepositoryError;

use super::DocumentService;
use super::brain_notice::ReopenedComment;
use crate::documents::error::DocumentsError;
use crate::documents::models::DocumentComment;
use crate::documents::repository::DocumentCommentRepository;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReopenCommentRequest {
    pub document: DocumentId,
    pub comment: DocumentCommentId,
}

impl DocumentService {
    pub async fn reopen_comment(
        &self,
        who: &AuthenticatedUser,
        request: ReopenCommentRequest,
        trace: &TraceId,
    ) -> Result<DocumentComment, DocumentsError> {
        let ReopenCommentRequest { document, comment } = request;
        let (document, comment) = (&document, &comment);
        let current = self
            .authorized_document(who, document, DocumentAction::Comment, trace)
            .await?;
        let resolved = self.comment_of(document, comment).await?;
        if resolved.resolved_at.is_none() {
            return Ok(resolved);
        }
        let mut tx = self.repos.documents.begin().await?;
        if !DocumentCommentRepository::reopen_in(&mut tx, document, comment, &who.id).await? {
            drop(tx);
            return self.comment_of(document, comment).await;
        }
        let reopened = ReopenedComment {
            document: &current,
            comment: &resolved,
            reopened_by: &who.id,
        };
        self.record_comment_reopened(&mut tx, reopened).await?;
        tx.commit().await.map_err(RepositoryError::from)?;
        let reopened = self.comment_of(document, comment).await?;
        let event = DocumentCommentReopenedEvent {
            document_id: document.clone(),
            comment_id: comment.clone(),
            reopened_by: who.id.clone(),
        };
        let to = self.recipients(&current).await;
        if let Err(error) = self.events.comment_reopened(&to, &event).await {
            tracing::warn!(document_id = %document, error = %error, "Comment event not relayed");
        }
        Ok(reopened)
    }
}
