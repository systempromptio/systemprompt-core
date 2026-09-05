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

use anyhow::anyhow;
use systemprompt_models::services::ProviderEntry;

use super::DispatchError;

// Why: returns an owned value because a minted token is short-lived and cannot
// borrow from the process-wide secret store the way a static key does.
pub(super) async fn resolve(provider: &ProviderEntry) -> Result<String, DispatchError> {
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

    match google::ServiceAccountKey::parse(secret) {
        Some(key) => google::access_token(provider.api_key_secret.as_str(), &key)
            .await
            .map_err(|e| {
                DispatchError::PreAudit(anyhow!(
                    "could not mint a Google access token from secret '{}': {e}",
                    provider.api_key_secret.as_str()
                ))
            }),
        None => Ok(secret.clone()),
    }
}
