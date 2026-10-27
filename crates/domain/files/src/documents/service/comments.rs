//! Comments: threads anchored to a stable node id and, optionally, the
//! `nth` occurrence of a quote inside that node.
//!
//! Commenting needs `comment`; reading the thread needs `view`. A reply
//! carries its parent's anchor. Because the anchor is a stable id, an edit
//! anywhere else leaves it in place; a commit that deletes the node or drops
//! the quoted text is refused unless it allows detaching, and a detached
//! comment keeps its node and loses its quote.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentCommentId, DocumentId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    DocumentAction, DocumentCommentCreatedEvent, DocumentCommentResolvedEvent, Projection,
    ProjectionNode,
};

use super::{CommentPolicy, DocumentService};
use crate::documents::error::DocumentsError;
use crate::documents::models::{CommentAnchor, DocumentComment, NewComment};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveCommentRequest {
    pub document: DocumentId,
    pub comment: DocumentCommentId,
    pub answering: Option<DocumentRevisionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentRequest {
    pub document: DocumentId,
    pub anchor: Option<CommentAnchor>,
    pub body: String,
    pub parent_id: Option<DocumentCommentId>,
}

fn node_text(node: &ProjectionNode) -> String {
    let mut text = node.text.clone().unwrap_or_default();
    for child in &node.children {
        let inner = node_text(child);
        if !inner.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&inner);
        }
    }
    text
}

fn anchor_holds(ids: &Projection, anchor: &CommentAnchor) -> bool {
    let Some(node) = ids.find(&anchor.node) else {
        return false;
    };
    anchor.find.as_deref().is_none_or(|find| {
        let nth = usize::try_from(anchor.nth.unwrap_or(0)).unwrap_or(0);
        node_text(node).matches(find).count() > nth
    })
}

impl DocumentService {
    pub async fn create_comment(
        &self,
        who: &AuthenticatedUser,
        request: CommentRequest,
        trace: &TraceId,
    ) -> Result<DocumentComment, DocumentsError> {
        let document = self
            .authorized_document(who, &request.document, DocumentAction::Comment, trace)
            .await?;
        self.check_comment_limit(&request.body)?;
        let anchor = match (&request.parent_id, request.anchor) {
            (Some(parent), _) => self.comment_of(&document.id, parent).await?.anchor,
            (None, Some(anchor)) => anchor,
            (None, None) => {
                return Err(DocumentsError::InvalidComment {
                    detail: "a comment needs a node anchor or a parent_id",
                });
            },
        };
        let head = self.resolve_revision(&document, None).await?;
        let ids = self.projected(&document, &head).await?.ids;
        if ids.find(&anchor.node).is_none() {
            return Err(DocumentsError::UnknownNode { node: anchor.node });
        }
        if !anchor_holds(&ids, &anchor) {
            return Err(DocumentsError::FindNone {
                find: anchor.find.unwrap_or_default(),
                closest: None,
            });
        }
        let comment = self
            .repos
            .comments
            .insert(NewComment {
                document_id: document.id.clone(),
                anchor,
                author_user_id: who.id.clone(),
                body: request.body,
                parent_id: request.parent_id,
            })
            .await?;
        let event = DocumentCommentCreatedEvent {
            document_id: document.id.clone(),
            comment_id: comment.id.clone(),
            node_id: comment.anchor.node.clone(),
            parent_id: comment.parent_id.clone(),
            author_id: comment.author_user_id.clone(),
        };
        let to = self.recipients(&document).await;
        if let Err(error) = self.events.comment_created(&to, &event).await {
            tracing::warn!(document_id = %document.id, error = %error, "Comment event not relayed");
        }
        Ok(comment)
    }

    pub async fn comments(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<Vec<DocumentComment>, DocumentsError> {
        self.authorized_document(who, document, DocumentAction::View, trace)
            .await?;
        Ok(self.repos.comments.list_for_document(document).await?)
    }

    pub async fn resolve_comment(
        &self,
        who: &AuthenticatedUser,
        request: ResolveCommentRequest,
        trace: &TraceId,
    ) -> Result<DocumentComment, DocumentsError> {
        let ResolveCommentRequest {
            document,
            comment,
            answering,
        } = request;
        let (document, comment) = (&document, &comment);
        let current = self
            .authorized_document(who, document, DocumentAction::Comment, trace)
            .await?;
        if !self
            .repos
            .comments
            .resolve(document, comment, &who.id)
            .await?
        {
            return Err(DocumentsError::CommentNotFound {
                document_id: document.clone(),
                comment_id: comment.clone(),
            });
        }
        if let Some(revision) = &answering {
            self.resolve_revision(&current, Some(revision)).await?;
            self.repos
                .comments
                .set_answering(document, comment, revision)
                .await?;
        }
        let resolved = self.comment_of(document, comment).await?;
        let event = DocumentCommentResolvedEvent {
            document_id: document.clone(),
            comment_id: comment.clone(),
            resolved_by: who.id.clone(),
        };
        let to = self.recipients(&current).await;
        if let Err(error) = self.events.comment_resolved(&to, &event).await {
            tracing::warn!(document_id = %document, error = %error, "Comment event not relayed");
        }
        Ok(resolved)
    }

    async fn comment_of(
        &self,
        document: &DocumentId,
        comment: &DocumentCommentId,
    ) -> Result<DocumentComment, DocumentsError> {
        self.repos
            .comments
            .list_for_document(document)
            .await?
            .into_iter()
            .find(|c| c.id == *comment)
            .ok_or_else(|| DocumentsError::CommentNotFound {
                document_id: document.clone(),
                comment_id: comment.clone(),
            })
    }

    pub(super) async fn affected_anchors(
        &self,
        document: &DocumentId,
        ids: &Projection,
        policy: &CommentPolicy,
    ) -> Result<Vec<DocumentCommentId>, DocumentsError> {
        let affected: Vec<DocumentCommentId> = self
            .repos
            .comments
            .list_for_document(document)
            .await?
            .into_iter()
            .filter(|c| c.resolved_at.is_none() && !anchor_holds(ids, &c.anchor))
            .map(|c| c.id)
            .collect();
        if !affected.is_empty() && !policy.allow_detach {
            return Err(DocumentsError::AnchorsAffected { comments: affected });
        }
        Ok(affected)
    }

    pub(super) async fn settle_comments(
        &self,
        document: &DocumentId,
        revision: &DocumentRevisionId,
        detached: &[DocumentCommentId],
        policy: &CommentPolicy,
    ) -> Result<(), DocumentsError> {
        if !detached.is_empty() {
            self.repos.comments.detach(document, detached).await?;
        }
        if let Some(comment) = &policy.answering
            && !self
                .repos
                .comments
                .set_answering(document, comment, revision)
                .await?
        {
            tracing::warn!(
                document_id = %document,
                comment_id = %comment,
                "Answered comment is not on this document; the revision stands"
            );
        }
        Ok(())
    }
}
