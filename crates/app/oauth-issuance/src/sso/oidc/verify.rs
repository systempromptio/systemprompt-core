//! `id_token` validation: the signature against the issuer's JWKS, the
//! registered claims, the nonce and the authorized party, then the mapping of
//! the configured claims onto an [`ExternalIdentity`].
//!
//! RS256 and ES256 are accepted: the key named by `kid` pins the algorithm,
//! and a header naming any other algorithm is refused before verification. No
//! issuer-specific claim is required, so a Keycloak realm, Entra ID or any
//! conforming issuer validates the same way.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use jsonwebtoken::{Validation, decode, decode_header};
// JSON: the claim set is issuer-defined and the configured claim names are
// read from it by name; `ExternalIdentity::raw_claims` keeps it as issued.
use serde_json::Value;
use sha2::{Digest, Sha256};
use systemprompt_identifiers::SsoAssertionId;
use systemprompt_security::keys::JwksClient;
use systemprompt_traits::sso::ExternalIdentity;

use super::config::OidcConfig;
use super::discovery::OidcDiscovery;
use super::error::OidcProviderError;

pub async fn validate_id_token(
    id_token: &str,
    cfg: &OidcConfig,
    discovery: &OidcDiscovery,
    jwks: &JwksClient,
    expected_nonce: &str,
) -> Result<ExternalIdentity, OidcProviderError> {
    let header = decode_header(id_token).map_err(OidcProviderError::IdTokenHeader)?;
    if header.crit.is_some() {
        return Err(OidcProviderError::CriticalHeader);
    }
    let kid = header.kid.ok_or(OidcProviderError::NoKid)?;
    let jwk = jwks
        .fetch_at(cfg.issuer(), &discovery.jwks_uri, &kid)
        .await
        .map_err(OidcProviderError::SigningKey)?;
    // Why: RFC 8725 §3.1: the algorithm comes from the key, never the header.
    if header.alg != jwk.algorithm() {
        return Err(OidcProviderError::Algorithm(header.alg));
    }
    let key = jwk
        .decoding_key()
        .map_err(OidcProviderError::KeyComponents)?;

    let mut validation = Validation::new(jwk.algorithm());
    validation.set_issuer(&[cfg.issuer()]);
    validation.set_audience(&[cfg.client_id.as_str()]);
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    validation.validate_nbf = true;
    validation.leeway = cfg.clock_skew_seconds;

    let claims = decode::<Value>(id_token, &key, &validation)
        .map_err(OidcProviderError::IdTokenRejected)?
        .claims;
    let mut identity = identity_from_claims(claims, cfg, expected_nonce)?;
    if identity.assertion_id.is_none() {
        identity.assertion_id = Some(SsoAssertionId::new(
            URL_SAFE_NO_PAD.encode(Sha256::digest(id_token.as_bytes())),
        ));
    }
    Ok(identity)
}

// JSON: the ID token's claim set is provider-defined; only the standard
// claims are read, the rest is passed through untyped.
pub fn identity_from_claims(
    claims: Value,
    cfg: &OidcConfig,
    expected_nonce: &str,
) -> Result<ExternalIdentity, OidcProviderError> {
    let nonce = claims.get("nonce").and_then(Value::as_str);
    if expected_nonce.is_empty() || nonce != Some(expected_nonce) {
        return Err(OidcProviderError::Nonce);
    }
    let audiences = match claims.get("aud") {
        Some(Value::Array(values)) => values.len(),
        Some(_) => 1,
        None => 0,
    };
    let azp = claims.get("azp").and_then(Value::as_str);
    // Why: OIDC Core §3.1.3.7 step 5 — a token for several audiences must name
    // this client as its authorized party.
    if audiences > 1 && azp != Some(cfg.client_id.as_str()) {
        return Err(OidcProviderError::AuthorizedParty);
    }

    let text = |name: &str| {
        claims
            .get(name)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let subject = text("sub").ok_or(OidcProviderError::NoSubject)?;
    let email = text(&cfg.claims.email);
    let display_name = text(&cfg.claims.name).or_else(|| text("preferred_username"));
    let email_verified = email.is_some() && claim_flag(claims.get("email_verified"));
    let groups = claim_strings(claims.get(&cfg.claims.groups));
    let assertion_id = text("jti").and_then(|jti| SsoAssertionId::try_new(jti).ok());
    let assertion_expires_at = claims
        .get("exp")
        .and_then(Value::as_i64)
        .and_then(|exp| DateTime::<Utc>::from_timestamp(exp, 0));

    Ok(ExternalIdentity {
        subject,
        email,
        email_verified,
        display_name,
        groups,
        groups_overage: false,
        tenant_id: None,
        assertion_id,
        assertion_expires_at,
        raw_claims: claims,
    })
}

// JSON: a provider claim that may be a bool or a string flag.
fn claim_flag(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::String(text)) => text.eq_ignore_ascii_case("true"),
        _ => false,
    }
}

// JSON: a provider claim that may be a string or an array of strings.
fn claim_strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Some(Value::String(single)) if !single.is_empty() => vec![single.clone()],
        _ => Vec::new(),
    }
}
