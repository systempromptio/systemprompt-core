//! Tool inputs: one struct per tool, each rendered as an object
//! `inputSchema`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Deserialize;
use systemprompt_files::documents::service::FolderMove;
use systemprompt_identifiers::{
    DocumentCommentId, DocumentFolderId, DocumentId, DocumentNodeId, DocumentOpId,
    DocumentProposalId, DocumentRevisionId, DocumentTemplateId, UserId,
};
use systemprompt_models::documents::{AccessLevel, DocumentFormat, EditMode, EditOp};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ListInput {
    #[serde(default)]
    pub scope: ListScopeInput,
    #[serde(default)]
    pub folder_id: Option<DocumentFolderId>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub sort: ListSortInput,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ListScopeInput {
    #[default]
    Folder,
    Shared,
    Mine,
    Recent,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ListSortInput {
    Name,
    #[default]
    Updated,
    Recent,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SearchInput {
    pub query: String,
    #[serde(default)]
    pub format: Option<DocumentFormat>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateInput {
    pub template_id: DocumentTemplateId,
    #[serde(default)]
    pub folder_id: Option<DocumentFolderId>,
    pub name: String,
    #[serde(default)]
    pub params: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub ops: Vec<EditOp>,
    #[serde(default)]
    pub message: Option<String>,
}

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

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct EditInput {
    pub document_id: DocumentId,
    #[serde(default)]
    pub base_revision: Option<DocumentRevisionId>,
    pub ops: Vec<EditOp>,
    #[serde(default)]
    pub mode: Option<EditMode>,
    #[serde(default)]
    pub op_id: Option<DocumentOpId>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub answering: Option<DocumentCommentId>,
    #[serde(default)]
    pub allow_detach: bool,
    #[serde(default)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct HistoryInput {
    pub document_id: DocumentId,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ShareInput {
    #[serde(default)]
    pub document_id: Option<DocumentId>,
    #[serde(default)]
    pub folder_id: Option<DocumentFolderId>,
    #[serde(default)]
    pub subject_kind: Option<String>,
    #[serde(default)]
    pub subject_value: Option<String>,
    #[serde(default)]
    pub level: Option<AccessLevel>,
    #[serde(default)]
    pub valid_until: Option<DateTime<Utc>>,
    #[serde(default)]
    pub transfer_to: Option<UserId>,
    #[serde(default)]
    pub revoke_all: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    #[default]
    Native,
    Pdf,
    Md,
    Html,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportAs {
    #[default]
    Url,
    Preview,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ExportInput {
    pub document_id: DocumentId,
    #[serde(default)]
    pub revision_id: Option<DocumentRevisionId>,
    #[serde(default)]
    pub format: ExportFormat,
    #[serde(default, rename = "as")]
    pub export_as: ExportAs,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GuideInput {
    pub topics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommentAction {
    Create,
    List,
    Resolve,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct QuoteAnchor {
    pub find: String,
    #[serde(default)]
    pub nth: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CommentInput {
    pub action: CommentAction,
    pub document_id: DocumentId,
    #[serde(default)]
    pub node: Option<DocumentNodeId>,
    #[serde(default)]
    pub anchor: Option<QuoteAnchor>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub parent_id: Option<DocumentCommentId>,
    #[serde(default)]
    pub comment_id: Option<DocumentCommentId>,
    #[serde(default)]
    pub answering: Option<DocumentRevisionId>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct DocumentInput {
    pub document_id: DocumentId,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CopyInput {
    pub document_id: DocumentId,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub folder_id: Option<DocumentFolderId>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateInput {
    pub document_id: DocumentId,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "folder_move")]
    #[schemars(with = "Option<DocumentFolderId>", description = "null = root")]
    pub folder_id: FolderMove,
}

fn folder_move<'de, D>(deserializer: D) -> Result<FolderMove, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<DocumentFolderId>::deserialize(deserializer).map(FolderMove::from_present)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProposalAction {
    #[default]
    List,
    Accept,
    Reject,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ProposalsInput {
    #[serde(default)]
    pub action: ProposalAction,
    #[serde(default)]
    pub document_id: Option<DocumentId>,
    #[serde(default)]
    pub proposal_id: Option<DocumentProposalId>,
    #[serde(default)]
    pub note: Option<String>,
}
