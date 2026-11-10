//! Application-owned credential resolution for Git source synchronization.
//!
//! A source's `credential_reference` is a label, not a secret name: it resolves
//! only the custom secret `git_source/<host>/<label>`, where `<host>` is the
//! source repository's authority. Secrets outside the `git_source/` namespace
//! are unreachable, and a credential stored for one host is never sent to
//! another.
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
                repository,
                credential_reference: Some(reference),
                ..
            } => {
                let key = git_credential_key(&repository, &reference)
                    .ok_or(OrchestrationError::CredentialUnresolved)?;
                let credential = self
                    .secrets
                    .secret(&key)
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

pub fn git_credential_key(repository: &str, reference: &str) -> Option<String> {
    let authority = repository
        .strip_prefix("https://")?
        .split('/')
        .next()
        .filter(|host| !host.is_empty())?
        .to_ascii_lowercase();
    if reference.is_empty() {
        return None;
    }
    Some(format!(
        "{GIT_CREDENTIAL_NAMESPACE}/{authority}/{reference}"
    ))
}

pub const GIT_CREDENTIAL_NAMESPACE: &str = "git_source";
