//! Personal provider keys: which providers take one, and storing one after an
//! upstream check proves it works.
//!
//! Both the gateway's `/v1/me/provider-credentials` route and any in-process
//! console call [`store_personal_key`], so a key is verified and sealed the
//! same way whichever surface the user added it from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::{ProviderEntry, ProviderRegistry};

use super::upstream::UpstreamTarget;
use crate::repository::{UserProviderCredentialMeta, UserProviderCredentialRepository};

const VERIFY_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_KEY_BYTES: usize = 4096;

#[derive(Debug, thiserror::Error)]
pub enum PersonalKeyError {
    #[error("provider '{0}' does not take a personal key")]
    NotPersonalProvider(String),
    #[error("{0} rejected the key; check it was copied whole and the subscription is active")]
    Rejected(String),
    #[error("{provider} could not verify the key (upstream answered {status}); try again shortly")]
    Unverified { provider: String, status: String },
    #[error("personal key could not be stored")]
    Internal(#[source] systemprompt_traits::BoxedSource),
}

/// The registry's `credential: user` providers, in declaration order.
#[must_use]
pub fn personal_providers(registry: &ProviderRegistry) -> Vec<ProviderEntry> {
    registry
        .providers
        .iter()
        .filter(|entry| entry.is_user_credential())
        .cloned()
        .collect()
}

pub fn personal_provider(
    registry: &ProviderRegistry,
    name: &str,
) -> Result<ProviderEntry, PersonalKeyError> {
    personal_providers(registry)
        .into_iter()
        .find(|entry| entry.name.as_str() == name)
        .ok_or_else(|| PersonalKeyError::NotPersonalProvider(name.to_owned()))
}

/// Verifies `key` against the provider upstream, then seals and stores it as
/// `user_id`'s key for `provider`. The key never appears in an error or log.
pub async fn store_personal_key(
    db: &DbPool,
    registry: &ProviderRegistry,
    user_id: &UserId,
    provider: &str,
    key: &str,
    label: Option<&str>,
) -> Result<UserProviderCredentialMeta, PersonalKeyError> {
    let entry = personal_provider(registry, provider)?;
    let key = key.trim();
    if key.is_empty() || key.len() > MAX_KEY_BYTES {
        return Err(PersonalKeyError::Rejected(provider_label(&entry)));
    }
    verify_upstream(&entry, key, user_id.as_str()).await?;
    let label = label.map(str::trim).filter(|l| !l.is_empty());
    let meta = UserProviderCredentialRepository::new(db)
        .set_key(user_id, entry.name.as_str(), key, label)
        .await
        .map_err(|e| PersonalKeyError::Internal(e.into()))?;
    tracing::info!(
        user_id = %user_id,
        provider = %entry.name.as_str(),
        "Personal provider key stored"
    );
    Ok(meta)
}

#[must_use]
pub fn provider_label(entry: &ProviderEntry) -> String {
    entry
        .display_name
        .clone()
        .unwrap_or_else(|| entry.name.as_str().to_owned())
}

// Why: OpenCode Go and every OpenAI- or Anthropic-compatible upstream answer
// `GET {endpoint}/models` for any valid key and 401/403 for a bad one, so it
// proves the key without spending a token of the subscription.
async fn verify_upstream(
    entry: &ProviderEntry,
    key: &str,
    user_id: &str,
) -> Result<(), PersonalKeyError> {
    let rejected = || PersonalKeyError::Rejected(provider_label(entry));
    let unverified = |status: String| PersonalKeyError::Unverified {
        provider: provider_label(entry),
        status,
    };
    let target = UpstreamTarget::resolve_personal(entry, key, user_id).map_err(|_e| rejected())?;
    let call = target.call().await.map_err(|_e| rejected())?;
    let url = format!("{}/models", call.endpoint().trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(VERIFY_TIMEOUT)
        .build()
        .map_err(|e| PersonalKeyError::Internal(e.into()))?;
    let mut request = client.get(&url);
    for (name, value) in call.headers(entry.wire) {
        if !name.eq_ignore_ascii_case("content-type") {
            request = request.header(name, value);
        }
    }
    let response = request
        .send()
        .await
        .map_err(|_e| unverified("no response".to_owned()))?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    if matches!(status.as_u16(), 401 | 403) {
        return Err(rejected());
    }
    Err(unverified(status.as_u16().to_string()))
}
