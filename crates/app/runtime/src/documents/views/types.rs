//! The plain structs the read views answer with, shared by the MCP tools
//! (as their `inputSchema` and `outputSchema`) and the HTTP routes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    ContentHash, DocumentId, DocumentNodeId, DocumentProposalId, DocumentRevisionId, UserId,
};
use systemprompt_models::documents::{
    AccessLevel, DocumentFormat, DocumentUri, PresenceSource, PresenceState,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReadView {
    Outline,
    #[default]
    Text,
    Range,
    Cells,
    Changes,
    Node,
    Search,
    Meta,
    Model,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct NodeRange {
    pub from_node: DocumentNodeId,
    pub to_node: DocumentNodeId,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ReadInput {
    pub document_id: DocumentId,
    #[serde(default)]
    pub view: ReadView,
    #[serde(default)]
    pub range: Option<NodeRange>,
    #[serde(default)]
    pub sheet: Option<String>,
    #[serde(default)]
    pub cells: Option<String>,
    #[serde(default)]
    pub revision_id: Option<DocumentRevisionId>,
    #[serde(default)]
    pub since_revision: Option<DocumentRevisionId>,
    #[serde(default)]
    pub node_id: Option<DocumentNodeId>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReadOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<DocumentUri>,
    pub revision_id: DocumentRevisionId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<ContentHash>,
    pub format: DocumentFormat,
    pub lines: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<DocumentMeta>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DocumentMeta {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_path: Option<String>,
    pub access: AccessLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<i64>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub editing: Vec<EditingNow>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct EditingNow {
    pub user_id: UserId,
    pub source: PresenceSource,
    pub state: PresenceState,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditOutput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<DocumentUri>,
    pub revision_id: DocumentRevisionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_revision_id: Option<DocumentRevisionId>,
    pub applied: usize,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub merged: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub replayed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_id: Option<DocumentProposalId>,
    pub written: Vec<String>,
}
