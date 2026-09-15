//! Application-owned credential resolution for Git import, sync and
//! verification.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OptimizationError;
use std::collections::BTreeMap;
use systemprompt_config::SecretsBootstrap;
use systemprompt_identifiers::{ManagedSourceId, UserId};
use systemprompt_marketplace::managed::{
    GitSyncRequest, GitSyncResult, ManagedRepository, SourceSpec,
};
use systemprompt_models::feedback::verification::{
    DependencyVerificationManifest, DependencyVerificationRequest,
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
    ) -> Result<GitSyncResult, OptimizationError> {
        let credential = self.credential(owner, &request.source_id).await?;
        Ok(self
            .managed
            .sync_git_source_with_credential(owner, request, credential.as_deref())
            .await?)
    }

    pub async fn verify(
        &self,
        owner: &UserId,
        request: &DependencyVerificationRequest,
    ) -> Result<DependencyVerificationManifest, OptimizationError> {
        request.validate().map_err(|error| {
            OptimizationError::Source(format!("Invalid dependency verification request: {error}"))
        })?;
        let mut credentials = BTreeMap::new();
        for revision in &request.revisions {
            if !credentials.contains_key(&revision.source_id)
                && let Some(credential) = self.credential(owner, &revision.source_id).await?
            {
                credentials.insert(revision.source_id.clone(), credential);
            }
        }
        Ok(self
            .managed
            .verify_git_dependencies(owner, request, &credentials)
            .await?)
    }

    async fn credential(
        &self,
        owner: &UserId,
        source: &ManagedSourceId,
    ) -> Result<Option<String>, OptimizationError> {
        match self.managed.get_source(owner, source).await? {
            SourceSpec::Git {
                credential_reference: Some(reference),
                ..
            } => {
                let secrets = SecretsBootstrap::get().map_err(|error| {
                    OptimizationError::Source(format!("Git credentials are unavailable: {error}"))
                })?;
                let credential = secrets
                    .get(&reference)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        OptimizationError::Source(
                            "Git credential reference is unresolved".to_owned(),
                        )
                    })?;
                Ok(Some(credential.clone()))
            },
            SourceSpec::Git {
                credential_reference: None,
                ..
            } => Ok(None),
            _ => Err(OptimizationError::Source(
                "Operation requires a registered Git source".to_owned(),
            )),
        }
    }
}
