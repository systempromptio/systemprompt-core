//! Application-owned credential resolution for Git source synchronization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use super::OrchestrationError;
use std::sync::Arc;

use systemprompt_identifiers::{ManagedSourceId, UserId};
use systemprompt_marketplace::managed::{
    GitSyncRequest, GitSyncResult, ManagedRepository, SourceSpec,
};
use systemprompt_traits::SecretStore;

#[derive(Debug, Clone)]
pub struct GitSourceOrchestrator {
    managed: ManagedRepository,
    scratch_root: PathBuf,
    secrets: Arc<dyn SecretStore>,
}

impl GitSourceOrchestrator {
    pub const fn new(
        managed: ManagedRepository,
        scratch_root: PathBuf,
        secrets: Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            managed,
            scratch_root,
            secrets,
        }
    }

    pub async fn synchronize(
        &self,
        owner: &UserId,
        request: &GitSyncRequest,
    ) -> Result<GitSyncResult, OrchestrationError> {
        let credential = self.credential(owner, &request.source_id).await?;
        Ok(self
            .managed
            .sync_git_source_with_credential(
                owner,
                request,
                credential.as_deref(),
                &self.scratch_root,
            )
            .await?)
    }

    async fn credential(
        &self,
        owner: &UserId,
        source: &ManagedSourceId,
    ) -> Result<Option<String>, OrchestrationError> {
        match self.managed.get_source(owner, source).await? {
            SourceSpec::Git {
                credential_reference: Some(reference),
                ..
            } => {
                let credential = self
                    .secrets
                    .secret(&reference)
                    .filter(|value| !value.expose().is_empty())
                    .ok_or(OrchestrationError::CredentialUnresolved)?;
                Ok(Some(credential.expose().to_owned()))
            },
            SourceSpec::Git {
                credential_reference: None,
                ..
            } => Ok(None),
            _ => Err(OrchestrationError::NotGitSource),
        }
    }
}
