//! Folder rows and the folder insert request. A trashed folder carries
//! `trashed_at`, stored in the `document_folders.deleted_at` column.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{DocumentFolderId, UserId};
use systemprompt_models::documents::AgentEditPolicy;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentFolder {
    pub id: DocumentFolderId,
    pub parent_id: Option<DocumentFolderId>,
    pub name: String,
    pub ancestor_ids: Vec<DocumentFolderId>,
    pub owner_user_id: UserId,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub trashed_at: Option<DateTime<Utc>>,
    pub agent_edit_policy: AgentEditPolicy,
}

#[derive(Debug, Clone)]
pub struct NewFolder {
    pub parent_id: Option<DocumentFolderId>,
    pub name: String,
    pub owner_user_id: UserId,
    pub created_by: UserId,
}
