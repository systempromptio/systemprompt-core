//! Comment threads as plain structs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Serialize;
use systemprompt_files::documents::models::{CellRange, DocumentComment};
use systemprompt_files::documents::{DocumentService, DocumentsError};
use systemprompt_identifiers::{
    AgentName, ClientId, ContextId, DocumentCommentId, DocumentId, DocumentNodeId,
    DocumentRevisionId, TraceId, UserId,
};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_traits::ContextDirectory;

use crate::documents::attribution::owned_context_names;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CommentAnchorItem {
    pub node: DocumentNodeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub find: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nth: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<CommentRangeItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cells: Option<CellRange>,
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
    pub reopened_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reopened_by: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answering_revision_id: Option<DocumentRevisionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via_agent: Option<AgentName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<ContextId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<ClientId>,
}

impl From<DocumentComment> for CommentItem {
    fn from(comment: DocumentComment) -> Self {
        Self {
            id: comment.id,
            anchor: CommentAnchorItem {
                node: comment.anchor.node,
                find: comment.anchor.find,
                nth: comment.anchor.nth,
                cells: comment.anchor.cells,
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
            reopened_at: comment.reopened_at,
            reopened_by: comment.reopened_by,
            answering_revision_id: comment.answering_revision_id,
            via_agent: comment.via_agent,
            context_id: comment.context_id,
            context_name: None,
            client_id: comment.client_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CommentOutput {
    pub comments: Vec<CommentItem>,
}

pub async fn comments_view(
    service: &DocumentService,
    contexts: &dyn ContextDirectory,
    who: &AuthenticatedUser,
    document: &DocumentId,
    trace: &TraceId,
) -> Result<CommentOutput, DocumentsError> {
    let comments = service.comments(who, document, trace).await?;
    let names = owned_context_names(
        contexts,
        &who.id,
        comments
            .iter()
            .filter_map(|comment| comment.context_id.as_ref()),
    )
    .await?;
    let comments = comments
        .into_iter()
        .map(|comment| {
            let context_name = comment
                .context_id
                .as_ref()
                .and_then(|id| names.get(id).cloned());
            CommentItem {
                context_name,
                ..CommentItem::from(comment)
            }
        })
        .collect();
    Ok(CommentOutput { comments })
}
