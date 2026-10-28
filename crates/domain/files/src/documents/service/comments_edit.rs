//! Comments v2 on the service: editing a comment's body and mentions (its
//! author only), soft deletion (its author, or a caller who may share the
//! document), and the checks a range and a mention list pass before a
//! comment stores them.
//!
//! A range is two engine cursors, kept opaque and bounded in size. Mentions
//! are checked, shared with and announced in `mentions`; only a user newly
//! mentioned by an edit is told. Edits and deletions are announced
//! as `document.comment.updated` and `document.comment.deleted` to the
//! document's recipients and its live topic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentCommentId, DocumentId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    DocumentAction, DocumentCommentDeletedEvent, DocumentCommentUpdatedEvent,
};

use super::DocumentService;
use super::mentions::{CommentOrigin, MentionInput};
use crate::documents::error::DocumentsError;
use crate::documents::models::{CommentRange, DocumentComment};
use crate::documents::repository::CommentBodyEdit;

const MAX_CURSOR_CHARS: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditCommentRequest {
    pub document: DocumentId,
    pub comment: DocumentCommentId,
    pub body: String,
    pub mentions: Vec<UserId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteCommentRequest {
    pub document: DocumentId,
    pub comment: DocumentCommentId,
}

pub(super) fn check_range(range: Option<&CommentRange>) -> Result<(), DocumentsError> {
    let bounded = |cursor: &str| !cursor.is_empty() && cursor.len() <= MAX_CURSOR_CHARS;
    match range {
        Some(range) if !bounded(&range.start) || !bounded(&range.end) => {
            Err(DocumentsError::InvalidComment {
                detail: "a range needs a start and an end cursor of at most 1024 bytes",
            })
        },
        _ => Ok(()),
    }
}

impl DocumentService {
    pub async fn edit_comment(
        &self,
        who: &AuthenticatedUser,
        request: EditCommentRequest,
        trace: &TraceId,
    ) -> Result<DocumentComment, DocumentsError> {
        self.edit_comment_from(who, request, &CommentOrigin::default(), trace)
            .await
    }

    pub async fn edit_comment_from(
        &self,
        who: &AuthenticatedUser,
        request: EditCommentRequest,
        origin: &CommentOrigin,
        trace: &TraceId,
    ) -> Result<DocumentComment, DocumentsError> {
        let document = self
            .authorized_document(who, &request.document, DocumentAction::Comment, trace)
            .await?;
        self.check_comment_limit(&request.body)?;
        let before = self.live_comment(&document.id, &request.comment).await?;
        if before.author_user_id != who.id {
            return Err(DocumentsError::InvalidComment {
                detail: "only a comment's author can edit it",
            });
        }
        let input = MentionInput {
            body: &request.body,
            passed: &request.mentions,
            existing: &before.mentions,
        };
        let plan = self
            .plan_mentions(who, &document, input, (origin, trace))
            .await?;
        let mentions = plan.mentions.clone();
        let updated = if plan.grants.users.is_empty() {
            self.repos
                .comments
                .update_body(
                    &document.id,
                    CommentBodyEdit {
                        comment: &request.comment,
                        author: &who.id,
                        body: &request.body,
                        mentions: &mentions,
                    },
                )
                .await?
        } else {
            self.repos
                .comments
                .update_body_granting(
                    &document.id,
                    &request.comment,
                    &request.body,
                    &mentions,
                    &plan.grants,
                )
                .await?
        };
        if !updated {
            return Err(missing(&document.id, &request.comment));
        }
        let event = DocumentCommentUpdatedEvent {
            document_id: document.id.clone(),
            comment_id: request.comment.clone(),
            edited_by: who.id.clone(),
            mentions: mentions.clone(),
        };
        let to = self.recipients(&document).await;
        if let Err(error) = self.events.comment_updated(&to, &event).await {
            tracing::warn!(document_id = %document.id, error = %error, "Comment event not relayed");
        }
        let fresh: Vec<UserId> = mentions
            .into_iter()
            .filter(|user| !before.mentions.contains(user))
            .collect();
        let edited = self.comment_of(&document.id, &request.comment).await?;
        self.announce_mentions(who, &document, &edited, &fresh, &plan.grants)
            .await;
        Ok(edited)
    }

    pub async fn delete_comment(
        &self,
        who: &AuthenticatedUser,
        request: DeleteCommentRequest,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        let document = self
            .authorized_document(who, &request.document, DocumentAction::Comment, trace)
            .await?;
        let comment = self.live_comment(&document.id, &request.comment).await?;
        if comment.author_user_id != who.id {
            self.authorized_document(who, &request.document, DocumentAction::Share, trace)
                .await?;
        }
        if !self
            .repos
            .comments
            .soft_delete(&document.id, &request.comment)
            .await?
        {
            return Err(missing(&document.id, &request.comment));
        }
        let event = DocumentCommentDeletedEvent {
            document_id: document.id.clone(),
            comment_id: request.comment,
            deleted_by: who.id.clone(),
        };
        let to = self.recipients(&document).await;
        if let Err(error) = self.events.comment_deleted(&to, &event).await {
            tracing::warn!(document_id = %document.id, error = %error, "Comment event not relayed");
        }
        Ok(())
    }

    async fn live_comment(
        &self,
        document: &DocumentId,
        comment: &DocumentCommentId,
    ) -> Result<DocumentComment, DocumentsError> {
        let found = self.comment_of(document, comment).await?;
        if found.deleted_at.is_some() {
            return Err(missing(document, comment));
        }
        Ok(found)
    }
}

fn missing(document: &DocumentId, comment: &DocumentCommentId) -> DocumentsError {
    DocumentsError::CommentNotFound {
        document_id: document.clone(),
        comment_id: comment.clone(),
    }
}
