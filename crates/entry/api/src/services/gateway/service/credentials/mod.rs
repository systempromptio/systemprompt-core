//! The credential the gateway presents to an upstream provider.
//!
//! For almost every provider the stored secret *is* the credential and is sent
//! verbatim. Google Vertex AI is the exception that forced this module to
//! exist: its endpoints reject API-key authentication as a class — "API keys
//! are not supported by this API", HTTP 401 `CREDENTIALS_MISSING` — and want an
//! OAuth access token that asserts a principal. Such a token is minted from a
//! service-account key, expires in an hour, and so is not the string the
//! operator stored.
//!
//! The two cases are told apart by the secret's own content rather than by a
//! catalog flag. A Google service-account key is a JSON document that names
//! itself in a `type` field (`"service_account"`), which is an explicit,
//! Google-defined self-description rather than a guess about shape; anything
//! that is not that document is an API key, exactly as before. This keeps the
//! provider catalog unchanged and means an operator who pastes a service
//! account gets the right behaviour without having to know a flag exists.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod google;

#[cfg(feature = "test-api")]
pub(super) mod test_api {
    pub async fn google_access_token(name: &str, secret: &str) -> anyhow::Result<Option<String>> {
        match super::google::ServiceAccountKey::parse(secret)? {
            Some(key) => super::google::access_token(name, &key).await.map(Some),
            None => Ok(None),
        }
    }

    pub fn google_token_uri(secret: &str) -> anyhow::Result<Option<String>> {
        Ok(super::google::ServiceAccountKey::parse(secret)?.map(|key| key.token_uri))
    }
}

use anyhow::anyhow;
use systemprompt_models::services::ProviderEntry;

use super::DispatchError;

// Why: Google API keys use x-goog-api-key; OAuth tokens use Authorization:
// Bearer.
#[derive(Debug, Clone)]
pub(super) struct Credential {
    pub(super) value: String,
    pub(super) is_bearer: bool,
}

pub(super) async fn resolve(provider: &ProviderEntry) -> Result<Credential, DispatchError> {
    let secrets = systemprompt_config::SecretsBootstrap::get()
        .map_err(|e| DispatchError::PreAudit(anyhow!("Secrets not available: {e}")))?;

    let secret = secrets
        .get(provider.api_key_secret.as_str())
        .ok_or_else(|| {
            DispatchError::PreAudit(anyhow!(
                "Gateway API key secret '{}' not configured",
                provider.api_key_secret.as_str()
            ))
        })?;

    let parsed = google::ServiceAccountKey::parse(secret).map_err(|e| {
        DispatchError::PreAudit(anyhow!(
            "secret '{}' declares a Google service account but is malformed: {e}",
            provider.api_key_secret.as_str()
        ))
    })?;

    match parsed {
        Some(key) => {
            let token = google::access_token(provider.api_key_secret.as_str(), &key)
                .await
                .map_err(|e| {
                    DispatchError::PreAudit(anyhow!(
                        "could not mint a Google access token from secret '{}': {e}",
                        provider.api_key_secret.as_str()
                    ))
                })?;
            Ok(Credential {
                value: token,
                is_bearer: true,
            })
        },
        None => Ok(Credential {
            value: secret.clone(),
            is_bearer: false,
        }),
    }
}
