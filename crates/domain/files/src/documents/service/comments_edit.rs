//! Comments v2 on the service: editing a comment's body and mentions (its
//! author only), soft deletion (its author, or a caller who may share the
//! document), and the checks a range and a mention list pass before a
//! comment stores them.
//!
//! A range is two engine cursors, kept opaque and bounded in size. A mention
//! names a user who holds access to the document; each newly mentioned user
//! receives `document.comment.mentioned`. Edits and deletions are announced
//! as `document.comment.updated` and `document.comment.deleted` to the
//! document's recipients and its live topic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentCommentId, DocumentId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    DocumentAction, DocumentCommentDeletedEvent, DocumentCommentMentionedEvent,
    DocumentCommentUpdatedEvent,
};

use super::DocumentService;
use crate::documents::error::DocumentsError;
use crate::documents::models::{CommentRange, Document, DocumentComment};
use crate::documents::repository::CommentBodyEdit;

const MAX_MENTIONS: usize = 20;
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
        let mentions = self.checked_mentions(&document, request.mentions).await?;
        let updated = self
            .repos
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
            .await?;
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
        self.notify_mentions(&document.id, &request.comment, &who.id, &fresh)
            .await;
        self.comment_of(&document.id, &request.comment).await
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

    pub(super) async fn checked_mentions(
        &self,
        document: &Document,
        mentions: Vec<UserId>,
    ) -> Result<Vec<UserId>, DocumentsError> {
        let mut unique: Vec<UserId> = Vec::with_capacity(mentions.len());
        for user in mentions {
            if !unique.contains(&user) {
                unique.push(user);
            }
        }
        if unique.len() > MAX_MENTIONS {
            return Err(DocumentsError::InvalidComment {
                detail: "a comment mentions at most 20 users",
            });
        }
        for user in &unique {
            if self.access.level_of(document, user).await?.is_none() {
                return Err(DocumentsError::InvalidComment {
                    detail: "a mentioned user holds no access to the document",
                });
            }
        }
        Ok(unique)
    }

    pub(super) async fn notify_mentions(
        &self,
        document: &DocumentId,
        comment: &DocumentCommentId,
        author: &UserId,
        mentioned: &[UserId],
    ) {
        for user in mentioned {
            let event = DocumentCommentMentionedEvent {
                document_id: document.clone(),
                comment_id: comment.clone(),
                mentioned_user_id: user.clone(),
                author_id: author.clone(),
            };
            if let Err(error) = self
                .events
                .comment_mentioned(std::slice::from_ref(user), &event)
                .await
            {
                tracing::warn!(document_id = %document, error = %error, "Mention event not relayed");
            }
        }
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
