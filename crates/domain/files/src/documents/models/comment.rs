//! Comment rows: a thread entry anchored to a stable node id and,
//! optionally, a quote inside that node.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    DocumentCommentId, DocumentId, DocumentNodeId, DocumentRevisionId, UserId,
};

/// Where a comment points: a node, and optionally the `nth` (0-based)
/// occurrence of `find` inside it. A detached comment keeps its node and
/// loses the quote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentAnchor {
    pub node: DocumentNodeId,
    pub find: Option<String>,
    pub nth: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentComment {
    pub id: DocumentCommentId,
    pub document_id: DocumentId,
    pub anchor: CommentAnchor,
    pub author_user_id: UserId,
    pub body: String,
    pub parent_id: Option<DocumentCommentId>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<UserId>,
    pub answering_revision_id: Option<DocumentRevisionId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewComment {
    pub document_id: DocumentId,
    pub anchor: CommentAnchor,
    pub author_user_id: UserId,
    pub body: String,
    pub parent_id: Option<DocumentCommentId>,
}
