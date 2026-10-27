//! Document rows, the insert request, listing filters and search hits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    DocumentFolderId, DocumentId, DocumentRevisionId, DocumentTemplateId, UserId,
};
use systemprompt_models::documents::DocumentFormat;

use super::ListCursor;

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
    pub deleted_at: Option<DateTime<Utc>>,
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
