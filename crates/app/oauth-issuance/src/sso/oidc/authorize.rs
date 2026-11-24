//! The authorization redirect: the PKCE challenge, the random correlation
//! tokens and the authorize URL sent to the issuer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use systemprompt_traits::sso::SsoReauth;
use uuid::Uuid;

use super::OidcConfig;

pub(super) fn random_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

#[must_use]
pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[derive(Debug, Clone, Copy)]
pub struct AuthorizeParams<'a> {
    pub redirect_uri: &'a str,
    pub state: &'a str,
    pub nonce: &'a str,
    pub code_challenge: &'a str,
    pub reauth: SsoReauth,
}

#[must_use]
pub fn build_authorize_url(
    authorization_endpoint: &str,
    cfg: &OidcConfig,
    params: AuthorizeParams<'_>,
) -> String {
    let AuthorizeParams {
        redirect_uri,
        state,
        nonce,
        code_challenge,
        reauth,
    } = params;
    let scope = cfg.scopes.join(" ");
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query
        .append_pair("client_id", cfg.client_id.as_str())
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", &scope)
        .append_pair("state", state)
        .append_pair("nonce", nonce)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256");
    if reauth.force {
        query.append_pair("prompt", "login");
    }
    if let Some(max_age) = reauth.max_age.filter(|max_age| *max_age >= 0) {
        query.append_pair("max_age", &max_age.to_string());
    }
    let query = query.finish();
    let separator = if authorization_endpoint.contains('?') {
        '&'
    } else {
        '?'
    };
    format!("{authorization_endpoint}{separator}{query}")
}
