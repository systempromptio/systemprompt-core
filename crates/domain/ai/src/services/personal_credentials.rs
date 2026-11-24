//! The caller-bound view of personal provider credentials the gateway's
//! `/v1/me/provider-credentials` routes read.
//!
//! The view holds the repository whose cipher opens users' personal keys,
//! so the `ai` member provides it sealed and confers it on `gateway-http`
//! alone. Every method takes the request's [`GatewayPrincipal`], which only
//! the kernel's gateway credential check builds, and reads or writes that
//! principal's rows only: no method takes a user id, so a holder cannot list,
//! store or remove another user's credential.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::router::GatewayPrincipal;
use systemprompt_identifiers::CredentialId;
use systemprompt_manifest::services::catalog::ResolvedCatalog;
use systemprompt_traits::{ProviderTokenCache, RepositoryError};

use super::personal_keys::{PersonalKeyError, PersonalKeyRequest, store_personal_key};
use crate::repository::{UserProviderCredentialMeta, UserProviderCredentialRepository};

/// The key a caller stores for one catalog credential.
#[derive(Debug, Clone, Copy)]
pub struct PersonalKeyEntry<'a> {
    pub credential: &'a str,
    pub key: &'a str,
    pub label: Option<&'a str>,
}

/// The caller-bound personal credential view, conferred on `gateway-http`.
#[derive(Clone)]
pub struct PersonalProviderCredentials {
    repository: UserProviderCredentialRepository,
    tokens: Arc<dyn ProviderTokenCache>,
}

impl std::fmt::Debug for PersonalProviderCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersonalProviderCredentials")
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl systemprompt_extension::router::PersonalCredentialNames for PersonalProviderCredentials {
    async fn credential_names(
        &self,
        caller: &GatewayPrincipal,
    ) -> Result<std::collections::BTreeSet<String>, RepositoryError> {
        Ok(self
            .list(caller)
            .await?
            .into_iter()
            .map(|meta| meta.credential)
            .collect())
    }
}

impl PersonalProviderCredentials {
    #[must_use]
    pub fn new(
        repository: UserProviderCredentialRepository,
        tokens: Arc<dyn ProviderTokenCache>,
    ) -> Self {
        Self { repository, tokens }
    }

    pub async fn list(
        &self,
        caller: &GatewayPrincipal,
    ) -> Result<Vec<UserProviderCredentialMeta>, RepositoryError> {
        self.repository.list_metadata(caller.user_id()).await
    }

    pub async fn store(
        &self,
        caller: &GatewayPrincipal,
        catalog: &ResolvedCatalog,
        entry: PersonalKeyEntry<'_>,
    ) -> Result<UserProviderCredentialMeta, PersonalKeyError> {
        store_personal_key(
            &self.tokens,
            &self.repository,
            catalog,
            PersonalKeyRequest {
                user_id: caller.user_id(),
                credential: entry.credential,
                key: entry.key,
                label: entry.label,
            },
        )
        .await
    }

    pub async fn remove(
        &self,
        caller: &GatewayPrincipal,
        credential: &CredentialId,
    ) -> Result<bool, RepositoryError> {
        self.repository
            .delete_key(caller.user_id(), credential)
            .await
    }
}
