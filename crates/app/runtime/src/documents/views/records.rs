//! Comment threads as plain structs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Serialize;
use systemprompt_files::documents::models::DocumentComment;
use systemprompt_files::documents::{DocumentService, DocumentsError};
use systemprompt_identifiers::{
    DocumentCommentId, DocumentId, DocumentNodeId, DocumentRevisionId, TraceId, UserId,
};
use systemprompt_models::auth::AuthenticatedUser;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CommentAnchorItem {
    pub node: DocumentNodeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub find: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nth: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<CommentRangeItem>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CommentRangeItem {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CommentItem {
    pub id: DocumentCommentId,
    pub anchor: CommentAnchorItem,
    pub author_user_id: UserId,
    pub body: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<DocumentCommentId>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answering_revision_id: Option<DocumentRevisionId>,
}

impl From<DocumentComment> for CommentItem {
    fn from(comment: DocumentComment) -> Self {
        Self {
            id: comment.id,
            anchor: CommentAnchorItem {
                node: comment.anchor.node,
                find: comment.anchor.find,
                nth: comment.anchor.nth,
                range: comment.range.map(|range| CommentRangeItem {
                    start: range.start,
                    end: range.end,
                }),
            },
            author_user_id: comment.author_user_id,
            body: comment.body,
            mentions: comment.mentions,
            parent_id: comment.parent_id,
            created_at: comment.created_at,
            edited_at: comment.edited_at,
            resolved_at: comment.resolved_at,
            resolved_by: comment.resolved_by,
            answering_revision_id: comment.answering_revision_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CommentOutput {
    pub comments: Vec<CommentItem>,
}

pub async fn comments_view(
    service: &DocumentService,
    who: &AuthenticatedUser,
    document: &DocumentId,
    trace: &TraceId,
) -> Result<CommentOutput, DocumentsError> {
    let comments = service.comments(who, document, trace).await?;
    Ok(CommentOutput {
        comments: comments.into_iter().map(CommentItem::from).collect(),
    })
}
