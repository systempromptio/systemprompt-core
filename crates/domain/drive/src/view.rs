//! Document rows as the agent and the shell list them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, DocumentId};
use systemprompt_models::documents::client::DriveDocument;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentItem {
    pub id: DocumentId,
    pub name: String,
    pub format: String,
    pub mime_type: String,
    pub folder_id: Option<DocumentFolderId>,
    pub updated_at: String,
    pub size_bytes: Option<u64>,
    pub my_access: Option<String>,
}

impl From<&DriveDocument> for DocumentItem {
    fn from(document: &DriveDocument) -> Self {
        Self {
            id: document.id.clone(),
            name: document.name.clone(),
            format: document.format.clone(),
            mime_type: document.mime_type.clone(),
            folder_id: document.folder_id.clone(),
            updated_at: document.updated_at.clone(),
            size_bytes: document.size_bytes,
            my_access: document.my_access.clone(),
        }
    }
}


#[must_use]
pub fn document_items(documents: &[DriveDocument]) -> Vec<DocumentItem> {
    documents.iter().map(DocumentItem::from).collect()
}
