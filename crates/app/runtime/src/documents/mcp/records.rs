//! Outputs of the comment, access and lifecycle tools.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Serialize;
use systemprompt_files::documents::models::{DocumentComment, DocumentGrant, GrantTarget};
use systemprompt_identifiers::{
    DocumentCommentId, DocumentFolderId, DocumentId, DocumentNodeId, DocumentRevisionId, UserId,
};
use systemprompt_mcp::McpOutputSchema;
use systemprompt_models::documents::AccessLevel;

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
pub struct GrantItem {
    pub grant_id: String,
    pub subject_kind: String,
    pub subject_value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub level: AccessLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<DateTime<Utc>>,
    pub granted_by: UserId,
    pub granted_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<DocumentFolderId>,
}

impl From<DocumentGrant> for GrantItem {
    fn from(grant: DocumentGrant) -> Self {
        Self {
            grant_id: grant.id.to_string(),
            folder_id: match grant.target {
                GrantTarget::Folder(folder) => Some(folder),
                GrantTarget::Document(_) => None,
            },
            display_name: Some(
                grant
                    .subject_display
                    .unwrap_or_else(|| grant.subject_value.clone()),
            ),
            subject_kind: grant.subject_kind,
            subject_value: grant.subject_value,
            level: grant.access_level,
            valid_until: grant.valid_until,
            granted_by: grant.granted_by,
            granted_at: grant.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AccessOutput {
    pub document_id: DocumentId,
    pub owner: UserId,
    pub my_access: AccessLevel,
    pub grants: Vec<GrantItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub inherited: Vec<GrantItem>,
}
documents_output!(AccessOutput, "documents_access");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DocumentAck {
    #[serde(flatten)]
    pub document: DocumentItem,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub trashed: bool,
}
documents_output!(DocumentAck, "documents_document");
