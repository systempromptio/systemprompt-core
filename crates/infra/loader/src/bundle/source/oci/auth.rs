//! Bearer-challenge handling for OCI registries.
//!
//! Only the standard `WWW-Authenticate: Bearer realm=…,service=…,scope=…`
//! flow is implemented. Registry-specific login endpoints are out of scope:
//! an unrecognised challenge is a fetch failure, not a silent downgrade to an
//! anonymous request.
//!
//! The credential itself never appears in an error or a log field. A secret
//! containing `:` is treated as `user:token` and sent as HTTP Basic, matching
//! how registries issue robot accounts; anything else is sent as a bearer
//! token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use serde::Deserialize;

use crate::bundle::error::{BundleError, BundleResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BearerChallenge {
    pub realm: String,
    pub service: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[serde(default)]
    token: Option<String>,

    #[serde(default)]
    access_token: Option<String>,
}

#[must_use]
pub fn parse_challenge(header: &str) -> Option<BearerChallenge> {
    let rest = header
        .strip_prefix("Bearer ")
        .or_else(|| header.strip_prefix("bearer "))?;
    let mut realm = None;
    let mut service = None;
    let mut scope = None;

    for part in rest.split(',') {
        let (key, value) = part.trim().split_once('=')?;
        let value = value.trim().trim_matches('"').to_owned();
        match key.trim() {
            "realm" => realm = Some(value),
            "service" => service = Some(value),
            "scope" => scope = Some(value),
            _ => {},
        }
    }
    realm.map(|realm| BearerChallenge {
        realm,
        service,
        scope,
    })
}

pub fn apply_credential(
    builder: reqwest::RequestBuilder,
    secret: Option<&str>,
) -> reqwest::RequestBuilder {
    match secret {
        None => builder,
        Some(raw) => match raw.split_once(':') {
            Some((user, password)) => {
                let encoded =
                    base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"));
                builder.header(reqwest::header::AUTHORIZATION, format!("Basic {encoded}"))
            },
            None => builder.bearer_auth(raw),
        },
    }
}

pub async fn fetch_token(
    client: &reqwest::Client,
    challenge: &BearerChallenge,
    secret: Option<&str>,
    source_name: &str,
) -> BundleResult<String> {
    let mut request = client.get(&challenge.realm);
    if let Some(service) = challenge.service.as_ref() {
        request = request.query(&[("service", service)]);
    }
    if let Some(scope) = challenge.scope.as_ref() {
        request = request.query(&[("scope", scope)]);
    }
    request = apply_credential(request, secret);

    let response = request
        .send()
        .await
        .map_err(|e| BundleError::fetch(source_name, e))?;
    if !response.status().is_success() {
        return Err(BundleError::Auth {
            source_name: source_name.to_owned(),
        });
    }

    let body: TokenResponse = response
        .json()
        .await
        .map_err(|e| BundleError::fetch(source_name, format!("token response: {e}")))?;
    body.token
        .or(body.access_token)
        .ok_or_else(|| BundleError::Auth {
            source_name: source_name.to_owned(),
        })
}
