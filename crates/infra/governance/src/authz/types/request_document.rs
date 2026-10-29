//! The `document.access` kind of [`AuthzContext`]: the action, the target
//! document or folder, and the folder ancestors a grant may be inherited
//! through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

use systemprompt_identifiers::error::IdValidationError;
use systemprompt_identifiers::{DocumentFolderId, DocumentId};
use systemprompt_models::documents::DocumentAction;

use super::request::AuthzContext;

impl AuthzContext {
    #[must_use]
    pub fn document_access(
        action: DocumentAction,
        document_id: Option<&DocumentId>,
        folder_id: Option<&DocumentFolderId>,
    ) -> Self {
        let mut payload = serde_json::Map::new();
        payload.insert("action".to_owned(), action.as_str().into());
        if let Some(document_id) = document_id {
            payload.insert("document_id".to_owned(), document_id.as_str().into());
        }
        if let Some(folder_id) = folder_id {
            payload.insert("folder_id".to_owned(), folder_id.as_str().into());
        }
        Self {
            kind: Cow::Borrowed(Self::DOCUMENT_ACCESS_KIND),
            payload: serde_json::Value::Object(payload),
        }
    }

    #[must_use]
    pub fn with_folder_ancestors(mut self, ancestors: &[DocumentFolderId]) -> Self {
        if ancestors.is_empty() || self.kind != Self::DOCUMENT_ACCESS_KIND {
            return self;
        }
        if let Some(map) = self.payload.as_object_mut() {
            map.insert(
                "folder_ancestors".to_owned(),
                ancestors
                    .iter()
                    .map(|id| serde_json::Value::from(id.as_str()))
                    .collect(),
            );
        }
        self
    }

    pub fn document_access_ancestors(&self) -> Result<Vec<DocumentFolderId>, IdValidationError> {
        if self.kind != Self::DOCUMENT_ACCESS_KIND {
            return Ok(Vec::new());
        }
        let Some(raw) = self.payload.get("folder_ancestors") else {
            return Ok(Vec::new());
        };
        let entries = raw
            .as_array()
            .ok_or_else(|| malformed("folder_ancestors is not an array"))?;
        entries
            .iter()
            .map(|entry| {
                entry
                    .as_str()
                    .ok_or_else(|| malformed("folder_ancestors entry is not a string"))
                    .and_then(DocumentFolderId::try_new)
            })
            .collect()
    }

    #[must_use]
    pub fn document_action(&self) -> Option<DocumentAction> {
        if self.kind != Self::DOCUMENT_ACCESS_KIND {
            return None;
        }
        self.payload
            .get("action")
            .and_then(|v| v.as_str())
            .and_then(|action| action.parse().ok())
    }

    #[must_use]
    pub fn document_access_document(&self) -> Option<DocumentId> {
        self.document_access_field("document_id")
            .and_then(|id| DocumentId::try_new(id).ok())
    }

    #[must_use]
    pub fn document_access_folder(&self) -> Option<DocumentFolderId> {
        self.document_access_field("folder_id")
            .and_then(|id| DocumentFolderId::try_new(id).ok())
    }

    fn document_access_field(&self, key: &str) -> Option<&str> {
        if self.kind != Self::DOCUMENT_ACCESS_KIND {
            return None;
        }
        self.payload.get(key).and_then(|v| v.as_str())
    }
}

fn malformed(message: &str) -> IdValidationError {
    IdValidationError::Invalid {
        id_type: "DocumentFolderId",
        message: message.to_owned(),
    }
}
