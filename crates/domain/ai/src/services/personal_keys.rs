//! Personal credentials: which ones take a member's own key, and storing one
//! after every endpoint it authenticates proves it works.
//!
//! A key is stored under the catalog credential id, so one stored key serves
//! every endpoint of every provider that names the credential. The gateway's
//! `/v1/me/provider-credentials` route and any in-process console call
//! [`store_personal_key`], so a key is verified and sealed one way.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::catalog::{
    ResolvedCatalog, ResolvedCredential, ResolvedEndpoint,
};
use systemprompt_security::credential::TokenCache;
use systemprompt_traits::ProviderTokenCache;

use super::upstream::{EndpointBinding, UpstreamCall, UpstreamTarget};
use crate::repository::{UserProviderCredentialMeta, UserProviderCredentialRepository};

const VERIFY_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_KEY_BYTES: usize = 4096;

#[derive(Debug, thiserror::Error)]
pub enum PersonalKeyError {
    #[error("credential '{0}' does not take a personal key")]
    NotPersonalCredential(String),
    #[error("{0} rejected the key; check it was copied whole and the subscription is active")]
    Rejected(String),
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
        .ok_or_else(|| PersonalKeyError::NotPersonalCredential(id.to_owned()))
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
        return Err(PersonalKeyError::Rejected(credential_label(credential)));
    }
    verify_personal_key(tokens, catalog, credential, key, user_id).await?;
    let label = label.map(str::trim).filter(|l| !l.is_empty());
    let sealed = credentials
        .cipher()
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
        return Err(PersonalKeyError::NotPersonalCredential(
            credential.id.as_str().to_owned(),
        ));
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

struct Probe<'a> {
    catalog: &'a ResolvedCatalog,
    binding: EndpointBinding<'a>,
    client: reqwest::Client,
    call: UpstreamCall,
    user_id: &'a UserId,
}

// Why: OpenCode Go and every OpenAI- or Anthropic-compatible upstream answer
// `GET {endpoint}/models` for any valid key and 401/403 for a bad one, so it
// proves the key without spending a token of the subscription.
async fn verify_endpoint(
    tokens: &Arc<TokenCache>,
    catalog: &ResolvedCatalog,
    binding: EndpointBinding<'_>,
    key: &str,
    user_id: &UserId,
) -> Result<(), PersonalKeyError> {
    let rejected = || PersonalKeyError::Rejected(credential_label(binding.credential));
    let target =
        UpstreamTarget::resolve_personal(tokens, binding, key, user_id).map_err(|_e| rejected())?;
    let call = target.call().await.map_err(|_e| rejected())?;
    let client = reqwest::Client::builder()
        .timeout(VERIFY_TIMEOUT)
        .build()
        .map_err(|e| PersonalKeyError::Internal(e.into()))?;
    let url = format!("{}/models", call.endpoint().trim_end_matches('/'));
    let mut request = client.get(&url);
    for (name, value) in call.headers(binding.endpoint.wire) {
        if !name.eq_ignore_ascii_case("content-type") {
            request = request.header(name, value);
        }
    }
    let response = request.send().await.map_err(|error| {
        tracing::warn!(
            endpoint = %binding.endpoint.endpoint_ref(),
            %error,
            "Personal key check got no response"
        );
        Verdict::Unverified("no response".to_owned()).into_error(binding.credential)
    })?;
    let status = response.status().as_u16();
    if matches!(status, 404 | 405) {
        let probe = Probe {
            catalog,
            binding,
            client,
            call,
            user_id,
        };
        return probe_completion(&probe).await;
    }
    classify(status, false).map_err(|verdict| verdict.into_error(binding.credential))
}

// Why: OpenCode Go serves no `/models` listing (404) and answers a malformed
// body with a 500 before it checks the key, so the only request that proves a
// key there is a well-formed one-token completion; a bad key is then a 401.
async fn probe_completion(probe: &Probe<'_>) -> Result<(), PersonalKeyError> {
    let Probe {
        catalog,
        binding,
        client,
        call,
        user_id,
    } = probe;
    let wire = binding.endpoint.wire;
    let Some(upstream_model) = probe_model(catalog, binding.endpoint) else {
        return Err(
            Verdict::Unverified("no model to probe".to_owned()).into_error(binding.credential)
        );
    };
    // JSON: provider request body — a one-token probe in the upstream wire format.
    let body = serde_json::json!({
        "model": upstream_model,
        "max_tokens": 1,
        "messages": [{"role": "user", "content": "ping"}],
    });
    let forward = vec![("x-opencode-session".to_owned(), probe_session(user_id))];
    let mut request = client
        .post(call.url(wire, &upstream_model, false))
        .body(body.to_string());
    for (name, value) in call.headers_forwarding(wire, &forward) {
        request = request.header(name, value);
    }
    let status = request
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(
                endpoint = %binding.endpoint.endpoint_ref(),
                %error,
                "Personal key probe got no response"
            );
            Verdict::Unverified("no response".to_owned()).into_error(binding.credential)
        })?
        .status()
        .as_u16();
    classify(status, true).map_err(|verdict| verdict.into_error(binding.credential))
}

fn probe_session(user_id: &UserId) -> String {
    format!(
        "ses_keycheck_{}",
        user_id
            .as_str()
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(20)
            .collect::<String>()
    )
}

fn probe_model(catalog: &ResolvedCatalog, endpoint: &ResolvedEndpoint) -> Option<String> {
    let target = endpoint.endpoint_ref();
    let mut models: Vec<_> = catalog.models().collect();
    models.sort_by_key(|model| !model.is_advertised());
    models.into_iter().find_map(|model| {
        catalog
            .deployments(model.id.as_str())
            .iter()
            .find(|hop| hop.endpoint_ref() == target)
            .map(|hop| hop.upstream_model.clone())
    })
}

enum Verdict {
    Rejected,
    Unverified(String),
}

impl Verdict {
    fn into_error(self, credential: &ResolvedCredential) -> PersonalKeyError {
        match self {
            Self::Rejected => PersonalKeyError::Rejected(credential_label(credential)),
            Self::Unverified(status) => PersonalKeyError::Unverified {
                credential: credential_label(credential),
                status,
            },
        }
    }
}

// Why: a completion probe that got past authentication proves the key even when
// the upstream then refuses the request itself (bad request, rate limited).
fn classify(status: u16, completion: bool) -> Result<(), Verdict> {
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err(Verdict::Rejected),
        400 | 422 | 429 if completion => Ok(()),
        other => Err(Verdict::Unverified(other.to_string())),
    }
}
