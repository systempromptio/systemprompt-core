//! Proving a personal key against each upstream endpoint it authenticates,
//! without spending more than one token of the subscription.
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

use super::personal_keys::{PersonalKeyError, credential_label};
use super::upstream::{EndpointBinding, UpstreamCall, UpstreamTarget};

const VERIFY_TIMEOUT: Duration = Duration::from_secs(10);

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
pub(super) async fn verify_endpoint(
    tokens: &Arc<TokenCache>,
    catalog: &ResolvedCatalog,
    binding: EndpointBinding<'_>,
    key: &str,
    user_id: &UserId,
) -> Result<(), PersonalKeyError> {
    let rejected = || PersonalKeyError::Rejected {
        credential: credential_label(binding.credential),
    };
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
        Verdict::Unverified {
            status: "no response".to_owned(),
        }
        .into_error(binding.credential)
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
        let verdict = Verdict::Unverified {
            status: "no model to probe".to_owned(),
        };
        return Err(verdict.into_error(binding.credential));
    };
    // JSON: provider request body — a one-token probe in the upstream wire format.
    let body = serde_json::json!({
        "model": upstream_model,
        "max_tokens": 1,
        "messages": [{"role": "user", "content": "ping"}],
    });
    let forward = vec![(
        systemprompt_identifiers::headers::OPENCODE_SESSION.to_owned(),
        probe_session(user_id),
    )];
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
            Verdict::Unverified {
                status: "no response".to_owned(),
            }
            .into_error(binding.credential)
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
    Unverified { status: String },
}

impl Verdict {
    fn into_error(self, credential: &ResolvedCredential) -> PersonalKeyError {
        match self {
            Self::Rejected => PersonalKeyError::Rejected {
                credential: credential_label(credential),
            },
            Self::Unverified { status } => PersonalKeyError::Unverified {
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
        other => Err(Verdict::Unverified {
            status: other.to_string(),
        }),
    }
}
