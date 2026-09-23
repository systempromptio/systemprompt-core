//! Application-owned credential resolution for Git source synchronization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OrchestrationError;
use systemprompt_config::SecretsBootstrap;
use systemprompt_identifiers::{ManagedSourceId, UserId};
use systemprompt_marketplace::managed::{
    GitSyncRequest, GitSyncResult, ManagedRepository, SourceSpec,
};

#[derive(Debug, Clone)]
pub struct GitSourceOrchestrator {
    managed: ManagedRepository,
}

impl GitSourceOrchestrator {
    pub const fn new(managed: ManagedRepository) -> Self {
        Self { managed }
    }

    pub async fn synchronize(
        &self,
        owner: &UserId,
        request: &GitSyncRequest,
    ) -> Result<GitSyncResult, OrchestrationError> {
        let credential = self.credential(owner, &request.source_id).await?;
        Ok(self
            .managed
            .sync_git_source_with_credential(owner, request, credential.as_deref())
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
                let secrets = SecretsBootstrap::get().map_err(|error| {
                    OrchestrationError::Source(format!("Git credentials are unavailable: {error}"))
                })?;
                let credential = secrets
                    .get(&reference)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        OrchestrationError::Source(
                            "Git credential reference is unresolved".to_owned(),
                        )
                    })?;
                Ok(Some(credential.clone()))
            },
            SourceSpec::Git {
                credential_reference: None,
                ..
            } => Ok(None),
            _ => Err(OrchestrationError::Source(
                "Operation requires a registered Git source".to_owned(),
            )),
        }
    }
}
