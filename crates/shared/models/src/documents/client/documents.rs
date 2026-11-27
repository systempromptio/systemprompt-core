//! Documents, folders, listings, revisions and the conversation context a write
//! names.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use systemprompt_identifiers::{
    ContextId, DocumentFolderId, DocumentId, DocumentRevisionId, UserId,
};

/// Which documents `GET /api/v1/documents` lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DriveScope {
    Mine,
    Shared,
    Recent,
    Folder,
}

impl DriveScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mine => "mine",
            Self::Shared => "shared",
            Self::Recent => "recent",
            Self::Folder => "folder",
        }
    }
}

/// A document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveDocument {
    pub id: DocumentId,
    #[serde(default)]
    pub folder_id: Option<DocumentFolderId>,
    pub name: String,
    pub format: String,
    pub mime_type: String,
    pub head_revision_id: DocumentRevisionId,
    #[serde(default)]
    pub template_id: Option<String>,
    pub owner_user_id: UserId,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub my_access: Option<String>,
}

/// A folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveFolder {
    pub id: DocumentFolderId,
    #[serde(default)]
    pub parent_id: Option<DocumentFolderId>,
    pub name: String,
    pub owner_user_id: UserId,
    #[serde(default)]
    pub agent_edit_policy: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A page of `GET /api/v1/documents`, or every visible folder from `GET
/// /api/v1/document-folders`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveListing {
    #[serde(default)]
    pub folders: Vec<DriveFolder>,
    #[serde(default)]
    pub documents: Vec<DriveDocument>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// A revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveRevision {
    pub id: DocumentRevisionId,
    pub document_id: DocumentId,
    #[serde(default)]
    pub parent_revision_id: Option<DocumentRevisionId>,
    pub content_sha256: String,
    pub author_user_id: UserId,
    pub actor_kind: String,
    pub origin_kind: String,
    #[serde(default)]
    pub message: Option<String>,
    pub created_at: String,
}

/// `GET /api/v1/documents/{id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveDetail {
    pub document: DriveDocument,
    pub head: DriveRevision,
    pub my_access: String,
    #[serde(default)]
    pub folder_path: Option<String>,
}

/// `201` of a create or an import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveCreated {
    pub document: DriveDocument,
    pub revision: DriveRevision,
}

/// `GET /{id}/revisions`, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveRevisions {
    #[serde(default)]
    pub revisions: Vec<DriveRevision>,
}

/// A conversation registered with the instance; its id is what a write's
/// `x-context-id` names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveContext {
    pub data: DriveContextData,
}

/// The body of [`DriveContext`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveContextData {
    pub context_id: ContextId,
    pub name: String,
}

/// Body of `POST /api/v1/document-folders`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateFolderRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<DocumentFolderId>,
}

/// Body of `POST /api/v1/documents` from a template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateDocumentRequest {
    pub template_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<DocumentFolderId>,
}
