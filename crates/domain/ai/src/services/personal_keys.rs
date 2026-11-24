//! Personal credentials: which ones take a member's own key, and storing one.
//!
//! A key is stored, under its catalog credential id, once every endpoint it
//! authenticates proves it works; the provider-credentials route calls
//! [`store_personal_key`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::{CredentialId, UserId};
use systemprompt_manifest::services::catalog::{
    ResolvedCatalog, ResolvedCredential, ResolvedEndpoint,
};
use systemprompt_security::credential::TokenCache;
use systemprompt_traits::ProviderTokenCache;

use super::personal_key_probe::verify_endpoint;
use super::upstream::EndpointBinding;
use crate::repository::{UserProviderCredentialMeta, UserProviderCredentialRepository};

const MAX_KEY_BYTES: usize = 4096;

#[derive(Debug, thiserror::Error)]
pub enum PersonalKeyError {
    #[error("credential '{credential}' does not take a personal key")]
    NotPersonalCredential { credential: CredentialId },
    #[error(
        "{credential} rejected the key; check it was copied whole and the subscription is active"
    )]
    Rejected { credential: String },
    #[error(
        "{credential} could not verify the key (upstream answered {status}); try again shortly"
    )]
    Unverified { credential: String, status: String },
    #[error("personal key could not be stored")]
    Internal(#[source] systemprompt_traits::BoxedSource),
}

#[must_use]
pub fn personal_credentials(catalog: &ResolvedCatalog) -> Vec<&ResolvedCredential> {
    catalog
        .credentials()
        .filter(|credential| credential.is_personal())
        .filter(|credential| !credential_endpoints(catalog, credential).is_empty())
        .collect()
}

pub fn personal_credential<'c>(
    catalog: &'c ResolvedCatalog,
    id: &str,
) -> Result<&'c ResolvedCredential, PersonalKeyError> {
    personal_credentials(catalog)
        .into_iter()
        .find(|credential| credential.id.as_str() == id)
        .ok_or_else(|| PersonalKeyError::NotPersonalCredential {
            credential: CredentialId::new(id),
        })
}

#[must_use]
pub fn credential_endpoints<'c>(
    catalog: &'c ResolvedCatalog,
    credential: &'c ResolvedCredential,
) -> Vec<&'c ResolvedEndpoint> {
    catalog
        .endpoints_for_credential(credential.id.as_str())
        .filter(|endpoint| {
            catalog
                .provider(endpoint.provider.as_str())
                .is_some_and(|provider| provider.enabled)
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
pub struct PersonalKeyRequest<'a> {
    pub user_id: &'a UserId,
    pub credential: &'a str,
    pub key: &'a str,
    pub label: Option<&'a str>,
}

pub async fn store_personal_key(
    tokens: &Arc<dyn ProviderTokenCache>,
    credentials: &UserProviderCredentialRepository,
    catalog: &ResolvedCatalog,
    request: PersonalKeyRequest<'_>,
) -> Result<UserProviderCredentialMeta, PersonalKeyError> {
    let PersonalKeyRequest {
        user_id,
        credential,
        key,
        label,
    } = request;
    let credential = personal_credential(catalog, credential)?;
    let key = key.trim();
    if key.is_empty() || key.len() > MAX_KEY_BYTES {
        return Err(PersonalKeyError::Rejected {
            credential: credential_label(credential),
        });
    }
    verify_personal_key(tokens, catalog, credential, key, user_id).await?;
    let label = label.map(str::trim).filter(|l| !l.is_empty());
    let cipher = credentials.cipher();
    let sealed = cipher
        .seal(key)
        .map_err(|e| PersonalKeyError::Internal(e.into()))?;
    let meta = credentials
        .set_key(user_id, &credential.id, &sealed, label)
        .await
        .map_err(|e| PersonalKeyError::Internal(e.into()))?;
    tracing::info!(
        user_id = %user_id,
        credential = %credential.id.as_str(),
        "Personal provider key stored"
    );
    Ok(meta)
}

pub async fn verify_personal_key(
    tokens: &Arc<dyn ProviderTokenCache>,
    catalog: &ResolvedCatalog,
    credential: &ResolvedCredential,
    key: &str,
    user_id: &UserId,
) -> Result<(), PersonalKeyError> {
    let tokens = TokenCache::from_handle(tokens)
        .ok_or_else(|| PersonalKeyError::Internal("foreign token cache".into()))?;
    let endpoints = credential_endpoints(catalog, credential);
    if endpoints.is_empty() {
        return Err(PersonalKeyError::NotPersonalCredential {
            credential: credential.id.clone(),
        });
    }
    for endpoint in endpoints {
        let binding = EndpointBinding::of_endpoint(catalog, &endpoint.endpoint_ref())
            .map_err(|e| PersonalKeyError::Internal(e.into()))?;
        verify_endpoint(&tokens, catalog, binding, key, user_id).await?;
    }
    Ok(())
}

#[must_use]
pub fn credential_label(credential: &ResolvedCredential) -> String {
    credential
        .label
        .clone()
        .unwrap_or_else(|| credential.id.as_str().to_owned())
}
