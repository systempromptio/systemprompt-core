//! The issuer's discovery document, the authorize URL and the code
//! redemption of the operator sign-in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use systemprompt_models::net::validate_outbound_url;

use super::login::OperatorLoginRequest;
use crate::error::{CloudError, CloudResult};

#[derive(Deserialize)]
pub(super) struct Discovery {
    pub(super) issuer: String,
    pub(super) authorization_endpoint: String,
    pub(super) token_endpoint: String,
}

#[derive(Deserialize)]
pub(super) struct TokenResponse {
    pub(super) id_token: Option<String>,
}

pub(super) async fn discover(client: &reqwest::Client, issuer: &str) -> CloudResult<Discovery> {
    let issuer = issuer.trim_end_matches('/');
    let url = format!("{issuer}/.well-known/openid-configuration");
    let response = client.get(&url).send().await?;
    if !response.status().is_success() {
        return Err(CloudError::operator_auth(format!(
            "discovery at {url} returned {}",
            response.status()
        )));
    }
    let discovery: Discovery = response.json().await?;
    if discovery.issuer.trim_end_matches('/') != issuer {
        return Err(CloudError::operator_auth(format!(
            "discovery names issuer '{}', expected '{issuer}'",
            discovery.issuer
        )));
    }
    for endpoint in [&discovery.authorization_endpoint, &discovery.token_endpoint] {
        validate_outbound_url(endpoint).map_err(|source| CloudError::OperatorEndpoint {
            endpoint: endpoint.clone(),
            source,
        })?;
    }
    Ok(discovery)
}

#[derive(Clone, Copy)]
pub(super) struct AuthorizeParams<'a> {
    pub(super) redirect_uri: &'a str,
    pub(super) state: &'a str,
    pub(super) nonce: &'a str,
    pub(super) challenge: &'a str,
}

pub(super) fn authorize_url(
    discovery: &Discovery,
    request: &OperatorLoginRequest,
    params: AuthorizeParams<'_>,
) -> String {
    let AuthorizeParams {
        redirect_uri,
        state,
        nonce,
        challenge,
    } = params;
    let scope = request.scopes.join(" ");
    let separator = if discovery.authorization_endpoint.contains('?') {
        '&'
    } else {
        '?'
    };
    format!(
        "{}{separator}response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&nonce={}\
         &code_challenge={}&code_challenge_method=S256",
        discovery.authorization_endpoint,
        urlencoding::encode(&request.client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(&scope),
        urlencoding::encode(state),
        urlencoding::encode(nonce),
        urlencoding::encode(challenge),
    )
}

#[derive(Clone, Copy)]
pub(super) struct CodeRedemption<'a> {
    pub(super) code: &'a str,
    pub(super) redirect_uri: &'a str,
    pub(super) verifier: &'a str,
}

pub(super) async fn redeem_code(
    client: &reqwest::Client,
    discovery: &Discovery,
    request: &OperatorLoginRequest,
    redemption: CodeRedemption<'_>,
) -> CloudResult<String> {
    let CodeRedemption {
        code,
        redirect_uri,
        verifier,
    } = redemption;
    let response = client
        .post(&discovery.token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", request.client_id.as_str()),
            ("code_verifier", verifier),
        ])
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<unreadable body: {e}>"));
        return Err(CloudError::operator_auth(format!(
            "token endpoint returned {status}: {}",
            body.chars().take(500).collect::<String>()
        )));
    }
    let tokens: TokenResponse = response.json().await?;
    let id_token = tokens
        .id_token
        .ok_or_else(|| CloudError::operator_auth("token response carried no id_token"))?;
    Ok(id_token)
}
