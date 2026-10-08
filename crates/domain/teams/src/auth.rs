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
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::RwLock;

use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use systemprompt_identifiers::TeamsAppId;
use systemprompt_manifest::services::teams::BOT_FRAMEWORK_OPENID_CONFIG_URL;

use crate::error::{TeamsError, TeamsResult};

const ISSUER: &str = "https://api.botframework.com";

pub const MAX_TIMESTAMP_SKEW_SECS: u64 = 60 * 5;

const JWKS_TTL_SECS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, Deserialize)]
pub struct ActivityClaims {
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

#[derive(Debug)]
pub struct ActivityTokenVerifier {
    http: reqwest::Client,
    audience: TeamsAppId,
    openid_config_url: String,
    cache: RwLock<Option<KeyCache>>,
}

impl ActivityTokenVerifier {
    #[must_use]
    pub fn new(http: reqwest::Client, app_id: TeamsAppId) -> Self {
        Self {
            http,
            audience: app_id,
            openid_config_url: BOT_FRAMEWORK_OPENID_CONFIG_URL.to_owned(),
            cache: RwLock::new(None),
        }
    }

    #[must_use]
    pub fn with_openid_url(
        http: reqwest::Client,
        app_id: TeamsAppId,
        openid_config_url: impl Into<String>,
    ) -> Self {
        Self {
            http,
            audience: app_id,
            openid_config_url: openid_config_url.into(),
            cache: RwLock::new(None),
        }
    }

    pub async fn verify(
        &self,
        token: &str,
        service_url: &str,
        now_unix: i64,
    ) -> TeamsResult<ActivityClaims> {
        let header = decode_header(token).map_err(|source| TeamsError::InvalidToken {
            context: "invalid token header",
            source,
        })?;
        let kid = header.kid.ok_or(TeamsError::MissingKeyId)?;

        let jwk = self.key_for(&kid, now_unix).await?;
        let key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e).map_err(|source| {
            TeamsError::InvalidToken {
                context: "malformed signing key",
                source,
            }
        })?;
        validate_token(token, &key, &self.audience, service_url)
    }

    async fn key_for(&self, kid: &str, now_unix: i64) -> TeamsResult<Jwk> {
        if let Some(jwk) = self.cached_key(kid, now_unix) {
            return Ok(jwk);
        }
        let keys = self.fetch_keys().await?;
        let jwk = keys.get(kid).cloned();
        match self.cache.write() {
            Ok(mut guard) => {
                *guard = Some(KeyCache {
                    keys,
                    refreshed_at_unix: now_unix,
                });
            },
            Err(e) => tracing::warn!(error = %e, "Teams signing-key cache lock is poisoned"),
        }
        jwk.ok_or_else(|| TeamsError::UnknownSigningKey {
            kid: kid.to_owned(),
        })
    }

    fn cached_key(&self, kid: &str, now_unix: i64) -> Option<Jwk> {
        let guard = self
            .cache
            .read()
            .inspect_err(|e| tracing::warn!(error = %e, "Teams signing-key cache lock is poisoned"))
            .ok()?;
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

pub fn validate_token(
    token: &str,
    key: &DecodingKey,
    audience: &TeamsAppId,
    service_url: &str,
) -> TeamsResult<ActivityClaims> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[audience.as_str()]);
    validation.validate_exp = true;
    validation.leeway = MAX_TIMESTAMP_SKEW_SECS;

    let data = decode::<ActivityClaims>(token, key, &validation).map_err(|e| match e.kind() {
        ErrorKind::ExpiredSignature => TeamsError::StaleToken,
        ErrorKind::InvalidIssuer => TeamsError::IssuerMismatch(ISSUER),
        ErrorKind::InvalidAudience => TeamsError::AudienceMismatch(audience.clone()),
        _ => TeamsError::InvalidToken {
            context: "token rejected",
            source: e,
        },
    })?;

    match data.claims.serviceurl.as_deref() {
        Some(claim) if claim == service_url => Ok(data.claims),
        Some(claim) => Err(TeamsError::ServiceUrlMismatch {
            claim: claim.to_owned(),
            activity: service_url.to_owned(),
        }),
        None => Err(TeamsError::MissingServiceUrl),
    }
}
