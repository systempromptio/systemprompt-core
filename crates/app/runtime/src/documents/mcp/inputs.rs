//! Tool inputs: one struct per tool, each rendered as an object
//! `inputSchema`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Deserialize;
use systemprompt_identifiers::{
    DocumentFolderId, DocumentId, DocumentNodeId, DocumentRevisionId, DocumentTemplateId,
};
use systemprompt_models::documents::{AccessLevel, CellValue, DocumentFormat, EditOp};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ListInput {
    #[serde(default)]
    pub folder_id: Option<DocumentFolderId>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub verbose: bool,
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
    pub verbose: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct EditInput {
    pub document_id: DocumentId,
    pub base_revision: DocumentRevisionId,
    pub ops: Vec<EditOp>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct StrReplaceInput {
    pub document_id: DocumentId,
    pub base_revision: DocumentRevisionId,
    pub old_str: String,
    pub new_str: String,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SetCellsInput {
    pub document_id: DocumentId,
    pub base_revision: DocumentRevisionId,
    pub sheet: String,
    pub range: String,
    pub values: Vec<Vec<CellValue>>,
    #[serde(default)]
    pub message: Option<String>,
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
    pub subject_kind: String,
    pub subject_value: String,
    pub level: AccessLevel,
    #[serde(default)]
    pub valid_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct RevisionInput {
    pub document_id: DocumentId,
    #[serde(default)]
    pub revision_id: Option<DocumentRevisionId>,
}
