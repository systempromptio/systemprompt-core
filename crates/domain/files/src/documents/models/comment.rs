//! Comment rows: a thread entry anchored to a stable node id and,
//! optionally, a quote inside that node and a range of engine cursors, or,
//! on a spreadsheet's sheet node, an A1 cell range.
//!
//! The range is two cursors of the live document's CRDT engine, opaque to
//! core: the editor resolves them, so the highlighted span follows
//! concurrent edits. A comment may mention users by id; it can be edited by
//! its author and is deleted softly, keeping its row for the thread.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    AgentName, ClientId, ContextId, DocumentCommentId, DocumentId, DocumentNodeId,
    DocumentRevisionId, TraceId, UserId,
};
use systemprompt_models::documents::CellRange;

/// Where a comment points: a node and what inside it.
///
/// Inside the node, the `nth` (0-based) occurrence of `find`, or, on a
/// sheet node, the `cells` of a range, which is exclusive with `find`,
/// `nth` and a cursor range. A detached comment keeps its node and loses
/// the quote and the cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentAnchor {
    pub node: DocumentNodeId,
    pub find: Option<String>,
    pub nth: Option<i32>,
    pub cells: Option<CellRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentRange {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentComment {
    pub id: DocumentCommentId,
    pub document_id: DocumentId,
    pub anchor: CommentAnchor,
    pub range: Option<CommentRange>,
    pub author_user_id: UserId,
    pub body: String,
    pub mentions: Vec<UserId>,
    pub parent_id: Option<DocumentCommentId>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<UserId>,
    pub reopened_at: Option<DateTime<Utc>>,
    pub reopened_by: Option<UserId>,
    pub answering_revision_id: Option<DocumentRevisionId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub via_agent: Option<AgentName>,
    pub context_id: Option<ContextId>,
    pub client_id: Option<ClientId>,
}

#[derive(Debug, Clone)]
pub struct NewComment {
    pub document_id: DocumentId,
    pub anchor: CommentAnchor,
    pub range: Option<CommentRange>,
    pub author_user_id: UserId,
    pub body: String,
    pub mentions: Vec<UserId>,
    pub parent_id: Option<DocumentCommentId>,
    pub via_agent: Option<AgentName>,
    pub context_id: Option<ContextId>,
    pub client_id: Option<ClientId>,
}

/// The users a comment shares the document with.
///
/// A comment shares it with users it mentions while they hold no access: each
/// gets a `comment` grant in the comment's own statement, audited as granted by
/// `granted_by` with the comment as the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionGrants {
    pub users: Vec<UserId>,
    pub granted_by: UserId,
    pub trace_id: TraceId,
    pub via_agent: Option<AgentName>,
}
