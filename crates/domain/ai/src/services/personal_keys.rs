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
    let status = response.status().as_u16();
    if matches!(status, 404 | 405) {
        return probe_completion(&client, &call, entry, user_id).await;
    }
    classify(status, false).map_err(|verdict| verdict.into_error(entry))
}

// Why: OpenCode Go serves no `/models` listing (404) and answers a malformed
// body with a 500 before it checks the key, so the only request that proves a
// key there is a well-formed one-token completion; a bad key is then a 401.
async fn probe_completion(
    client: &reqwest::Client,
    call: &super::upstream::UpstreamCall,
    entry: &ProviderEntry,
    user_id: &str,
) -> Result<(), PersonalKeyError> {
    let Some(model) = entry.models.iter().find(|m| !m.hidden) else {
        return Err(Verdict::Unverified("no model to probe".to_owned()).into_error(entry));
    };
    let upstream_model = model.effective_upstream_model(model.id.as_str());
    // JSON: provider request body — a one-token probe in the upstream wire format.
    let body = serde_json::json!({
        "model": upstream_model,
        "max_tokens": 1,
        "messages": [{"role": "user", "content": "ping"}],
    });
    let session = format!(
        "ses_keycheck_{}",
        user_id
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(20)
            .collect::<String>()
    );
    let forward = vec![("x-opencode-session".to_owned(), session)];
    let mut request = client
        .post(call.url(entry.wire, upstream_model, false))
        .body(body.to_string());
    for (name, value) in call.headers_forwarding(entry.wire, &forward) {
        request = request.header(name, value);
    }
    let status = request
        .send()
        .await
        .map_err(|_e| Verdict::Unverified("no response".to_owned()).into_error(entry))?
        .status()
        .as_u16();
    classify(status, true).map_err(|verdict| verdict.into_error(entry))
}

enum Verdict {
    Rejected,
    Unverified(String),
}

impl Verdict {
    fn into_error(self, entry: &ProviderEntry) -> PersonalKeyError {
        match self {
            Self::Rejected => PersonalKeyError::Rejected(provider_label(entry)),
            Self::Unverified(status) => PersonalKeyError::Unverified {
                provider: provider_label(entry),
                status,
            },
        }
    }
}

// A completion probe that got past authentication proves the key even when
// the upstream then refuses the request itself (bad request, rate limited).
fn classify(status: u16, completion: bool) -> Result<(), Verdict> {
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err(Verdict::Rejected),
        400 | 422 | 429 if completion => Ok(()),
        other => Err(Verdict::Unverified(other.to_string())),
    }
}
