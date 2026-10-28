//! Drive blob reads by content hash.
//!
//! A bridge downloads the bytes a change or manifest entry names by hash. A
//! blob is served only when the caller can view at least one live document
//! that has a revision with that hash, so a hash alone never opens content.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_identifiers::{ContentHash, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentAction;
use systemprompt_traits::{ByteRange, StoredFileRead};

use super::DocumentService;
use crate::documents::access::{AccessCheck, AccessTarget};
use crate::documents::error::DocumentsError;

const BLOB_REVISION_SCAN: i64 = 50;

impl DocumentService {
    pub async fn drive_blob(
        &self,
        who: &AuthenticatedUser,
        hash: &ContentHash,
        range: Option<ByteRange>,
        trace: &TraceId,
    ) -> Result<StoredFileRead, DocumentsError> {
        let revisions = self
            .repos
            .revisions
            .with_content(hash, BLOB_REVISION_SCAN)
            .await?;
        let pairs = self.access.subject_pairs(who).await?;
        let mut checked = HashSet::new();
        for revision in revisions {
            if !checked.insert(revision.document_id.clone()) {
                continue;
            }
            let target = AccessTarget::Document(revision.document_id.clone());
            let check = AccessCheck {
                who,
                target: &target,
                action: DocumentAction::View,
                trace,
            };
            match self.access.probe_with(check, &pairs).await {
                Ok(granted) if granted.document.is_some() => {
                    return Ok(self.blobs.open(hash, range).await?);
                },
                Ok(_) | Err(DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }) => {
                },
                Err(other) => return Err(other),
            }
        }
        Err(DocumentsError::not_found("blob", hash))
    }
}
