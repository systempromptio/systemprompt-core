//! Tool inputs for copying, renaming and moving documents, folders, imports
//! and proposal review; re-exported through `inputs`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::Deserialize;
use systemprompt_files::documents::service::FolderMove;
use systemprompt_identifiers::{ContentHash, DocumentFolderId, DocumentId, DocumentProposalId};
use systemprompt_models::documents::DocumentFormat;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FolderAction {
    Create,
    Update,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct FolderInput {
    pub action: FolderAction,
    #[serde(default)]
    pub folder_id: Option<DocumentFolderId>,
    #[serde(default, deserialize_with = "folder_move")]
    #[schemars(with = "Option<DocumentFolderId>", description = "null = root")]
    pub parent_id: FolderMove,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ImportInput {
    pub blob_ref: ContentHash,
    pub name: String,
    #[serde(default)]
    pub folder_id: Option<DocumentFolderId>,
    #[serde(default)]
    pub format: Option<DocumentFormat>,
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
