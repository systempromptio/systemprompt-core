//! Comments, mentions, the node outline comments anchor to, and the people the
//! Drive names.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::notifications::Attention;

use systemprompt_identifiers::{DocumentCommentId, DocumentRevisionId, UserId};

/// The node a comment is pinned to, and the quoted text in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveAnchor {
    pub node: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub find: Option<String>,
}

/// `GET /{id}/read?view=outline`: one line per node, `<node id>:<hash8>|
/// <text>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveOutline {
    pub revision_id: DocumentRevisionId,
    pub format: String,
    #[serde(default)]
    pub lines: Vec<String>,
}

/// One node line of an outline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutlineLine<'a> {
    pub node: &'a str,
    pub hash: &'a str,
    pub text: &'a str,
}

impl DriveOutline {
    pub fn nodes(&self) -> impl Iterator<Item = OutlineLine<'_>> {
        self.lines
            .iter()
            .map(String::as_str)
            .filter_map(parse_outline_line)
    }
}

#[must_use]
pub fn parse_outline_line(line: &str) -> Option<OutlineLine<'_>> {
    let (head, text) = line.split_once('|')?;
    let (node, hash) = head.rsplit_once(':')?;
    if node.is_empty() || hash.is_empty() {
        return None;
    }
    Some(OutlineLine {
        node,
        hash,
        text: text.strip_prefix(' ').unwrap_or(text),
    })
}

/// A comment; replies carry `parent_id` and come in the same list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveComment {
    pub id: DocumentCommentId,
    #[serde(default)]
    pub anchor: Option<DriveAnchor>,
    pub author_user_id: UserId,
    pub body: String,
    #[serde(default)]
    pub parent_id: Option<DocumentCommentId>,
    pub created_at: String,
    #[serde(default)]
    pub resolved_at: Option<String>,
    #[serde(default)]
    pub resolved_by: Option<String>,
}

/// `GET /{id}/comments`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveComments {
    #[serde(default)]
    pub comments: Vec<DriveComment>,
}

/// `GET /api/v1/documents/people` (0.84).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrivePeople {
    #[serde(default)]
    pub people: Vec<DrivePerson>,
}

/// A person the Drive can name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrivePerson {
    pub user_id: UserId,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub access: Option<String>,
}

/// A mention option; the comment body must carry the mention as
/// `@[Name](user:<id>)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MentionOption {
    pub user_id: UserId,
    pub attention: Attention,
}

/// Body of `POST /api/v1/documents/{id}/comments`: a new comment on a node, or
/// a reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentRequest {
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<DriveAnchor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<DocumentCommentId>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mention_options: Vec<MentionOption>,
}
