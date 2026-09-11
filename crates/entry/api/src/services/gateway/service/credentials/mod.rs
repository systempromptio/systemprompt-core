//! The credential the gateway presents to an upstream provider.
//!
//! This module is a thin view over [`systemprompt_security::credential`]: it
//! looks the secret up in the store, hands it to
//! [`ProviderCredential::parse`], and asks the result for the header to send
//! and the endpoint to send it to. It decides nothing about credential types
//! itself, which is the point — a new credential kind is a new variant in the
//! security crate and no change here at all.
//!
//! For almost every provider the stored secret *is* the credential and is sent
//! verbatim. Google Vertex AI is the exception that forced the model to exist:
//! its endpoints reject API-key authentication as a class — "API keys are not
//! supported by this API", HTTP 401 `CREDENTIALS_MISSING` — and want an OAuth
//! access token that asserts a principal, minted from a service-account key
//! and expiring in an hour.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod google;

use anyhow::anyhow;
use systemprompt_models::services::ProviderEntry;
use systemprompt_security::credential::{
    CredentialError, CredentialScope, ProviderCredential, fill_endpoint,
};

use super::DispatchError;

// Why: Google API keys use x-goog-api-key; OAuth tokens use Authorization:
// Bearer. The adapters need that one bit, not the credential itself.
#[derive(Debug, Clone)]
pub(super) struct Credential {
    pub(super) value: String,
    pub(super) is_bearer: bool,
    pub(super) scope: CredentialScope,
}

pub fn fill_project(endpoint: &str, project: Option<&str>) -> Result<String, CredentialError> {
    let scope = CredentialScope {
        project: project.map(str::to_owned),
        ..CredentialScope::empty()
    };
    fill_endpoint(endpoint, &scope)
}

pub(super) async fn resolve(provider: &ProviderEntry) -> Result<Credential, DispatchError> {
    let secrets = systemprompt_config::SecretsBootstrap::get()
        .map_err(|e| DispatchError::PreAudit(anyhow!("Secrets not available: {e}")))?;

    let secret_name = provider.api_key_secret.as_str();
    let secret = secrets.get(secret_name).ok_or_else(|| {
        DispatchError::PreAudit(anyhow!(
            "Gateway API key secret '{secret_name}' not configured"
        ))
    })?;

    let credential = ProviderCredential::parse(secret).map_err(|e| {
        DispatchError::PreAudit(anyhow!(
            "secret '{secret_name}' declares a Google service account but is malformed: {e}"
        ))
    })?;

    let header = credential.bearer(secret_name).await.map_err(|e| {
        DispatchError::PreAudit(anyhow!(
            "could not mint a Google access token from secret '{secret_name}': {e}"
        ))
    })?;

    let is_bearer = header.is_bearer();
    Ok(Credential {
        value: header.value,
        is_bearer,
        scope: credential.scope(),
    })
}
