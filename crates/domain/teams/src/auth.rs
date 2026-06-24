//! Inbound Bot Framework activity-token validation.
//!
//! Every activity the Azure Bot Service delivers carries an
//! `Authorization: Bearer <JWT>` signed by the Bot Connector. There is no
//! static signing secret (unlike Slack's HMAC): the token is an RS256 JWT
//! validated against the Bot Connector's published JWKS. Validation asserts the
//! signature, the issuer (`https://api.botframework.com`), the audience (the bot's
//! Microsoft App Id), expiry within a tolerance window, and that the token's
//! `serviceurl` claim matches the activity's `serviceUrl` — binding the reply
//! target so a forged activity cannot redirect outbound replies.
//!
//! Signing keys are fetched from the `OpenID` metadata and cached in-process,
//! refreshed on a key-id miss (rotation) or after a TTL.

use std::collections::HashMap;
use std::sync::RwLock;

use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;

use crate::error::{TeamsError, TeamsResult};

const OPENID_CONFIG_URL: &str = "https://login.botframework.com/v1/.well-known/openidconfiguration";
const ISSUER: &str = "https://api.botframework.com";

/// Accepted expiry drift, mirroring the Slack signature skew window.
pub const MAX_TIMESTAMP_SKEW_SECS: u64 = 60 * 5;

/// How long a fetched key set is trusted before a refresh is forced.
const JWKS_TTL_SECS: i64 = 24 * 60 * 60;

/// Claims extracted after signature/issuer/audience/expiry are validated.
#[derive(Debug, Clone, Deserialize)]
pub struct ActivityClaims {
    /// The Bot Connector service URL the token is bound to.
    #[serde(default)]
    pub serviceurl: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenIdConfig {
    jwks_uri: String,
}

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Debug, Clone, Deserialize)]
struct Jwk {
    kid: String,
    n: String,
    e: String,
}

#[derive(Debug)]
struct KeyCache {
    keys: HashMap<String, Jwk>,
    refreshed_at_unix: i64,
}

/// Validates inbound activity tokens for one app registration.
#[derive(Debug)]
pub struct ActivityTokenVerifier {
    http: reqwest::Client,
    audience: String,
    openid_config_url: String,
    cache: RwLock<Option<KeyCache>>,
}

impl ActivityTokenVerifier {
    #[must_use]
    pub fn new(http: reqwest::Client, app_id: impl Into<String>) -> Self {
        Self {
            http,
            audience: app_id.into(),
            openid_config_url: OPENID_CONFIG_URL.to_owned(),
            cache: RwLock::new(None),
        }
    }

    /// Build a verifier whose `OpenID` metadata endpoint is overridden, so a
    /// test can serve the config + JWKS from a loopback mock server.
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn with_openid_url(
        http: reqwest::Client,
        app_id: impl Into<String>,
        openid_config_url: impl Into<String>,
    ) -> Self {
        Self {
            http,
            audience: app_id.into(),
            openid_config_url: openid_config_url.into(),
            cache: RwLock::new(None),
        }
    }

    /// Validate a bearer token against the channel's `service_url`, returning
    /// the extracted claims. `now_unix` drives the JWKS cache TTL (token
    /// expiry is checked against the system clock with a tolerance window).
    pub async fn verify(
        &self,
        token: &str,
        service_url: &str,
        now_unix: i64,
    ) -> TeamsResult<ActivityClaims> {
        let header = decode_header(token)
            .map_err(|e| TeamsError::TokenValidation(format!("invalid token header: {e}")))?;
        let kid = header
            .kid
            .ok_or_else(|| TeamsError::TokenValidation("token missing kid".to_owned()))?;

        let jwk = self.key_for(&kid, now_unix).await?;
        let key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
            .map_err(|e| TeamsError::TokenValidation(format!("malformed signing key: {e}")))?;
        validate_token(token, &key, &self.audience, service_url)
    }

    async fn key_for(&self, kid: &str, now_unix: i64) -> TeamsResult<Jwk> {
        if let Some(jwk) = self.cached_key(kid, now_unix) {
            return Ok(jwk);
        }
        let keys = self.fetch_keys().await?;
        let jwk = keys.get(kid).cloned();
        if let Ok(mut guard) = self.cache.write() {
            *guard = Some(KeyCache {
                keys,
                refreshed_at_unix: now_unix,
            });
        }
        jwk.ok_or_else(|| TeamsError::TokenValidation(format!("unknown signing key '{kid}'")))
    }

    fn cached_key(&self, kid: &str, now_unix: i64) -> Option<Jwk> {
        let guard = self.cache.read().ok()?;
        let jwk = guard.as_ref().and_then(|cache| {
            if now_unix - cache.refreshed_at_unix >= JWKS_TTL_SECS {
                None
            } else {
                cache.keys.get(kid).cloned()
            }
        });
        drop(guard);
        jwk
    }

    async fn fetch_keys(&self) -> TeamsResult<HashMap<String, Jwk>> {
        let config: OpenIdConfig = self
            .http
            .get(&self.openid_config_url)
            .send()
            .await?
            .json()
            .await?;
        let jwks: Jwks = self.http.get(&config.jwks_uri).send().await?.json().await?;
        Ok(jwks.keys.into_iter().map(|k| (k.kid.clone(), k)).collect())
    }
}

/// Validate a token against a known signing key, audience, and `service_url`.
///
/// This is the network-free core of [`ActivityTokenVerifier::verify`],
/// separated so the cryptographic and claim checks are testable without
/// fetching the JWKS.
pub fn validate_token(
    token: &str,
    key: &DecodingKey,
    audience: &str,
    service_url: &str,
) -> TeamsResult<ActivityClaims> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[audience]);
    validation.validate_exp = true;
    validation.leeway = MAX_TIMESTAMP_SKEW_SECS;

    let data = decode::<ActivityClaims>(token, key, &validation).map_err(|e| match e.kind() {
        ErrorKind::ExpiredSignature => TeamsError::StaleToken,
        ErrorKind::InvalidIssuer => TeamsError::IssuerMismatch(ISSUER.to_owned()),
        ErrorKind::InvalidAudience => TeamsError::AudienceMismatch(audience.to_owned()),
        _ => TeamsError::TokenValidation(e.to_string()),
    })?;

    match data.claims.serviceurl.as_deref() {
        Some(claim) if claim == service_url => Ok(data.claims),
        Some(claim) => Err(TeamsError::TokenValidation(format!(
            "serviceurl claim '{claim}' does not match activity serviceUrl '{service_url}'"
        ))),
        None => Err(TeamsError::TokenValidation(
            "token missing serviceurl claim".to_owned(),
        )),
    }
}
