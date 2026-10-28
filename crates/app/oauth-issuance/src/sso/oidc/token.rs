//! The authorization-code redemption at the issuer's token endpoint
//! (`client_secret_post`, with the PKCE verifier).
//!
//! Response bodies are never logged: a 2xx one holds tokens, and only the
//! `error` code of a refusal is surfaced.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use systemprompt_models::net::validate_outbound_url;

use super::error::OidcProviderError;

pub(super) struct CodeRedemption<'a> {
    pub token_endpoint: &'a str,
    pub code: &'a str,
    pub redirect_uri: &'a str,
    pub client_id: &'a str,
    pub client_secret: &'a str,
    pub code_verifier: &'a str,
}

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(default)]
    id_token: Option<String>,
}

#[derive(Deserialize)]
struct TokenRefusal {
    #[serde(default)]
    error: String,
}

pub(super) async fn redeem_code(
    http: &reqwest::Client,
    req: &CodeRedemption<'_>,
) -> Result<String, OidcProviderError> {
    let token_endpoint = validate_outbound_url(req.token_endpoint).map_err(|source| {
        OidcProviderError::Endpoint {
            field: "token_endpoint",
            source,
        }
    })?;
    let response = http
        .post(token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", req.code),
            ("redirect_uri", req.redirect_uri),
            ("client_id", req.client_id),
            ("client_secret", req.client_secret),
            ("code_verifier", req.code_verifier),
        ])
        .send()
        .await
        .and_then(|response| {
            if response.status().is_server_error() {
                response.error_for_status()
            } else {
                Ok(response)
            }
        })
        .map_err(OidcProviderError::TokenEndpoint)?;

    let status = response.status();
    if !status.is_success() {
        let refusal = response
            .json::<TokenRefusal>()
            .await
            .map(|refusal| refusal.error)
            .unwrap_or_default();
        return Err(OidcProviderError::CodeRefused {
            status,
            error: refusal,
        });
    }
    response
        .json::<TokenResponse>()
        .await
        .map_err(OidcProviderError::TokenEndpoint)?
        .id_token
        .filter(|token| !token.is_empty())
        .ok_or(OidcProviderError::NoIdToken)
}
