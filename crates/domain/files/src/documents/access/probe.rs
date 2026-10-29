//! The listing probe for a folder: the authz hook's `probe` over the folder
//! and the folders above it, with no audit row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_governance::authz::{AuthzContext, AuthzDecision, EntityRef};
use systemprompt_identifiers::{DocumentFolderId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentAction;

use super::request::{HookTarget, authz_request};
use super::service::{DocumentAccessService, root_folder_entity};
use crate::documents::error::DocumentsError;
use crate::documents::models::SubjectPairs;

/// The policy inputs a folder probe reads, gathered once by a caller that
/// probes many folders.
#[derive(Debug, Clone, Copy)]
pub struct FolderProbe<'a> {
    pub folder: Option<&'a DocumentFolderId>,
    pub pairs: &'a SubjectPairs,
    pub ancestors: &'a [DocumentFolderId],
}

impl DocumentAccessService {
    pub async fn probe_folder(
        &self,
        who: &AuthenticatedUser,
        folder: Option<&DocumentFolderId>,
        trace: &TraceId,
    ) -> bool {
        let (pairs, ancestors) = match self.folder_probe_inputs(who, folder).await {
            Ok(inputs) => inputs,
            Err(error) => {
                tracing::warn!(
                    folder_id = folder.map(DocumentFolderId::as_str),
                    error = %error,
                    "Document listing withheld a folder whose policy inputs were not read"
                );
                return false;
            },
        };
        let probe = FolderProbe {
            folder,
            pairs: &pairs,
            ancestors: &ancestors,
        };
        self.probe_folder_with(who, probe, trace).await
    }

    pub async fn probe_folder_with(
        &self,
        who: &AuthenticatedUser,
        probe: FolderProbe<'_>,
        trace: &TraceId,
    ) -> bool {
        let FolderProbe {
            folder,
            pairs,
            ancestors,
        } = probe;
        let entity = folder.cloned().unwrap_or_else(root_folder_entity);
        let request = authz_request(HookTarget {
            who,
            pairs,
            entity: EntityRef::Folder(entity),
            context: AuthzContext::document_access(DocumentAction::View, None, folder)
                .with_folder_ancestors(ancestors),
            trace,
            actor: self.actor(),
        });
        match self.hook.probe(request).await {
            AuthzDecision::Allow => true,
            AuthzDecision::Deny { reason, policy } => {
                tracing::debug!(
                    folder_id = folder.map(DocumentFolderId::as_str),
                    policy = %policy,
                    reason = %reason,
                    "Document listing withheld a folder the authz hook denies"
                );
                false
            },
        }
    }

    async fn folder_probe_inputs(
        &self,
        who: &AuthenticatedUser,
        folder: Option<&DocumentFolderId>,
    ) -> Result<(SubjectPairs, Vec<DocumentFolderId>), DocumentsError> {
        let pairs = self.subject_pairs(who).await?;
        let ancestors = match folder {
            Some(folder) => self.ancestors_nearest_first(folder).await?,
            None => Vec::new(),
        };
        Ok((pairs, ancestors))
    }

    pub async fn ancestors_nearest_first(
        &self,
        folder: &DocumentFolderId,
    ) -> Result<Vec<DocumentFolderId>, DocumentsError> {
        Ok(self
            .folders
            .ancestors(folder)
            .await?
            .into_iter()
            .rev()
            .collect())
    }
}
