//! Which of many documents the caller may view: the same check a read
//! makes, without the audit row, for an extension that labels or filters
//! ids it holds. Under an agent scope the agent's cap applies, so a
//! document whose cap is `none` is left out.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentAction;

use super::DocumentService;
use crate::documents::access::{AccessCheck, AccessTarget};
use crate::documents::error::DocumentsError;

pub const MAX_VISIBILITY_BATCH: usize = 500;

impl DocumentService {
    pub async fn visible_documents(
        &self,
        who: &AuthenticatedUser,
        documents: &[DocumentId],
        trace: &TraceId,
    ) -> Result<Vec<DocumentId>, DocumentsError> {
        if documents.len() > MAX_VISIBILITY_BATCH {
            return Err(DocumentsError::InvalidParams {
                detail: "at most 500 document ids per visibility check",
            });
        }
        let pairs = self.access.subject_pairs(who).await?;
        let mut visible = Vec::with_capacity(documents.len());
        for id in documents {
            let target = AccessTarget::Document(id.clone());
            let check = AccessCheck {
                who,
                target: &target,
                action: DocumentAction::View,
                trace,
            };
            match self.access.probe_with(check, &pairs).await {
                Ok(_) => visible.push(id.clone()),
                Err(
                    DocumentsError::Forbidden { .. }
                    | DocumentsError::NotFound { .. }
                    | DocumentsError::Trashed { .. }
                    | DocumentsError::Agent(_),
                ) => {},
                Err(other) => return Err(other),
            }
        }
        Ok(visible)
    }
}
