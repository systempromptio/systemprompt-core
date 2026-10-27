//! Document rows, the insert request, listing filters and search hits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    DocumentFolderId, DocumentId, DocumentRevisionId, DocumentTemplateId, UserId,
};
use systemprompt_models::documents::{AccessLevel, DocumentFormat};

use super::{ListCursor, ListingCursor};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub id: DocumentId,
    pub folder_id: Option<DocumentFolderId>,
    pub name: String,
    pub format: DocumentFormat,
    pub mime_type: String,
    pub head_revision_id: Option<DocumentRevisionId>,
    pub template_id: Option<DocumentTemplateId>,
    pub owner_user_id: UserId,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub trashed_at: Option<DateTime<Utc>>,
    pub trashed_by: Option<UserId>,
}

#[derive(Debug, Clone)]
pub struct NewDocument {
    pub folder_id: Option<DocumentFolderId>,
    pub name: String,
    pub format: DocumentFormat,
    pub mime_type: String,
    pub template_id: Option<DocumentTemplateId>,
    pub owner_user_id: UserId,
    pub created_by: UserId,
}

#[derive(Debug, Clone)]
pub struct VisibleListing {
    pub folder_id: Option<DocumentFolderId>,
    pub query: Option<String>,
    pub cursor: Option<ListCursor>,
    pub limit: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSearchHit {
    pub document_id: DocumentId,
    pub revision_id: DocumentRevisionId,
    pub snippet: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ListScope {
    #[default]
    Folder,
    Shared,
    Mine,
    Recent,
}

impl ListScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::Shared => "shared",
            Self::Mine => "mine",
            Self::Recent => "recent",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ListSort {
    Name,
    #[default]
    Updated,
    Recent,
}

impl ListSort {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Updated => "updated",
            Self::Recent => "recent",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScopedListing {
    pub scope: ListScope,
    pub folder_id: Option<DocumentFolderId>,
    pub query: Option<String>,
    pub sort: ListSort,
    pub cursor: Option<ListingCursor>,
    pub limit: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedDocument {
    pub document: Document,
    pub size_bytes: Option<i64>,
    pub my_access: AccessLevel,
    pub touched_at: Option<DateTime<Utc>>,
}
