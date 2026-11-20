//! Whether a document carries agent content: the flag the rendered view
//! picks its tier by, for a caller already authorised to view the document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentId;

use super::DocumentService;
use crate::documents::error::DocumentsError;

impl DocumentService {
    pub async fn agent_content(&self, document: &DocumentId) -> Result<bool, DocumentsError> {
        Ok(self.repos.documents.agent_content(document).await?)
    }
}
