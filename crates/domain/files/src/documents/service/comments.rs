//! Comments: threads anchored to a stable node id and, optionally, the
//! `nth` occurrence of a quote inside that node and a range of engine
//! cursors. A comment may mention users, and shares the document with a
//! mentioned user who cannot see it (`mentions`); editing and deletion live
//! in `comments_edit`. A deleted comment leaves listings and anchor checks.
//!
//! Commenting needs `comment`; reading the thread needs `view`. A reply
//! carries its parent's anchor. Because the anchor is a stable id, an edit
//! anywhere else leaves it in place; a commit that deletes the node or drops
//! the quoted text is refused unless it allows detaching, and a detached
//! comment keeps its node and loses its quote.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{
    DocumentCommentId, DocumentId, DocumentRevisionId, TraceId, UserId,
};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    DocumentAction, DocumentCommentCreatedEvent, DocumentCommentResolvedEvent,
};

use super::DocumentService;
use super::comment_anchors::anchor_holds;
use super::comments_edit::check_range;
use super::mentions::{CommentOrigin, MentionInput};
use super::notify::{MentionContext, MentionNotice, MentionOption, check_mention_options};
use systemprompt_traits::RepositoryError;

use crate::documents::error::DocumentsError;
use crate::documents::models::{
    CommentAnchor, CommentRange, Document, DocumentComment, MentionGrants, NewComment,
};

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
    pub range: Option<CommentRange>,
    pub body: String,
    pub mentions: Vec<UserId>,
    pub mention_options: Vec<MentionOption>,
    pub parent_id: Option<DocumentCommentId>,
}

impl DocumentService {
    pub async fn create_comment(
        &self,
        who: &AuthenticatedUser,
        request: CommentRequest,
        trace: &TraceId,
    ) -> Result<DocumentComment, DocumentsError> {
        self.create_comment_from(who, request, &CommentOrigin::default(), trace)
            .await
    }

    pub async fn create_comment_from(
        &self,
        who: &AuthenticatedUser,
        request: CommentRequest,
        origin: &CommentOrigin,
        trace: &TraceId,
    ) -> Result<DocumentComment, DocumentsError> {
        let document = self
            .authorized_document(who, &request.document, DocumentAction::Comment, trace)
            .await?;
        self.check_comment_limit(&request.body)?;
        check_range(request.range.as_ref())?;
        let input = MentionInput {
            body: &request.body,
            passed: &request.mentions,
            existing: &[],
        };
        let plan = self
            .plan_mentions(who, &document, input, (origin, trace))
            .await?;
        check_mention_options(self, &request.mention_options, &plan.mentions)?;
        let (anchor, range) = self
            .comment_anchor(
                &document,
                request.parent_id.as_ref(),
                request.anchor,
                request.range,
            )
            .await?;
        let new = NewComment {
            document_id: document.id.clone(),
            anchor,
            range,
            author_user_id: who.id.clone(),
            body: request.body,
            mentions: plan.mentions,
            parent_id: request.parent_id,
            via_agent: origin.via_agent.clone(),
        };
        let context = MentionContext {
            who,
            document: &document,
            granted: &plan.grants.users,
            via_agent: origin.via_agent.as_ref(),
            options: &request.mention_options,
        };
        let comment = self.store_comment(new, &plan.grants, context).await?;
        let event = DocumentCommentCreatedEvent {
            document_id: document.id.clone(),
            comment_id: comment.id.clone(),
            node_id: comment.anchor.node.clone(),
            parent_id: comment.parent_id.clone(),
            author_id: comment.author_user_id.clone(),
            mentions: comment.mentions.clone(),
        };
        let to = self.recipients(&document).await;
        if let Err(error) = self.events.comment_created(&to, &event).await {
            tracing::warn!(document_id = %document.id, error = %error, "Comment event not relayed");
        }
        self.announce_mentions(who, &document, &comment, &comment.mentions, &plan.grants)
            .await;
        Ok(comment)
    }

    async fn store_comment(
        &self,
        new: NewComment,
        grants: &MentionGrants,
        context: MentionContext<'_>,
    ) -> Result<DocumentComment, DocumentsError> {
        let mut tx = self.repos.documents.begin().await?;
        let comment = if grants.users.is_empty() {
            self.repos.comments.insert(&mut tx, new).await?
        } else {
            self.repos
                .comments
                .insert_granting(&mut tx, new, grants)
                .await?
        };
        self.record_mentions(
            &mut tx,
            MentionNotice {
                context,
                comment: &comment.id,
                body: &comment.body,
                anchor_quote: comment.anchor.find.as_deref(),
                mentions: &comment.mentions,
                notify: &comment.mentions,
                occurred_at: comment.created_at,
            },
        )
        .await?;
        tx.commit().await.map_err(RepositoryError::from)?;
        Ok(comment)
    }

    async fn comment_anchor(
        &self,
        document: &Document,
        parent: Option<&DocumentCommentId>,
        anchor: Option<CommentAnchor>,
        range: Option<CommentRange>,
    ) -> Result<(CommentAnchor, Option<CommentRange>), DocumentsError> {
        let (anchor, range) = match (parent, anchor) {
            (Some(parent), _) => {
                let parent = self.comment_of(&document.id, parent).await?;
                (parent.anchor, parent.range)
            },
            (None, Some(anchor)) => (anchor, range),
            (None, None) => {
                return Err(DocumentsError::InvalidComment {
                    detail: "a comment needs a node anchor or a parent_id",
                });
            },
        };
        let head = self.resolve_revision(document, None).await?;
        let ids = self.projected(document, &head).await?.ids;
        if ids.find(&anchor.node).is_none() {
            return Err(DocumentsError::UnknownNode { node: anchor.node });
        }
        if !anchor_holds(&ids, &anchor) {
            return Err(DocumentsError::FindNone {
                find: anchor.find.unwrap_or_default(),
                closest: None,
            });
        }
        Ok((anchor, range))
    }

    pub async fn comments(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<Vec<DocumentComment>, DocumentsError> {
        self.authorized_document(who, document, DocumentAction::View, trace)
            .await?;
        Ok(self
            .repos
            .comments
            .list_for_document(document)
            .await?
            .into_iter()
            .filter(|comment| comment.deleted_at.is_none())
            .collect())
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

    pub(super) async fn comment_of(
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
}
