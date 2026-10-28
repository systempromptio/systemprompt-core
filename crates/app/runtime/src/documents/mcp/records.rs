//! Outputs of the comment and lifecycle tools.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Serialize;
use systemprompt_files::documents::models::DocumentComment;
use systemprompt_identifiers::{DocumentCommentId, DocumentNodeId, DocumentRevisionId, UserId};
use systemprompt_mcp::McpOutputSchema;

use super::outputs::{DocumentItem, documents_output};

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CommentAnchorItem {
    pub node: DocumentNodeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub find: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nth: Option<i32>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CommentItem {
    pub id: DocumentCommentId,
    pub anchor: CommentAnchorItem,
    pub author_user_id: UserId,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<DocumentCommentId>,
    pub created_at: DateTime<Utc>,
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
            },
            author_user_id: comment.author_user_id,
            body: comment.body,
            parent_id: comment.parent_id,
            created_at: comment.created_at,
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
documents_output!(CommentOutput, "documents_comment");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DocumentAck {
    #[serde(flatten)]
    pub document: DocumentItem,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub trashed: bool,
}
documents_output!(DocumentAck, "documents_document");
