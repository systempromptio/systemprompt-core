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

// Why: an unknown kid arrives on an unauthenticated inbound request, so a
// rotation refetch is allowed at most once per this window.
const KID_MISS_REFETCH_SECS: i64 = 60;

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
    // JSON: each key is read on its own so one of another type or shape
    // (RFC 7517 §5) drops out instead of failing the whole set.
    keys: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct PublishedKey {
    #[serde(default)]
    kty: Option<String>,
    #[serde(rename = "use", default)]
    use_: Option<String>,
    #[serde(default)]
    alg: Option<String>,
    kid: String,
    n: String,
    e: String,
}

impl Jwks {
    fn into_signing_keys(self) -> HashMap<String, Jwk> {
        self.keys
            .into_iter()
            .filter_map(|raw| serde_json::from_value::<PublishedKey>(raw).ok())
            .filter(|k| {
                k.kty.as_deref() == Some("RSA")
                    && k.use_.as_deref().is_none_or(|u| u == "sig")
                    && k.alg.as_deref().is_none_or(|a| a == "RS256")
            })
            .map(|k| (k.kid.clone(), Jwk { n: k.n, e: k.e }))
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Jwk {
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
        if header.crit.is_some() {
            return Err(TeamsError::InvalidToken {
                context: "unsupported crit header",
                source: ErrorKind::InvalidToken.into(),
            });
        }
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
        if self.refetched_within(now_unix, KID_MISS_REFETCH_SECS) {
            return Err(TeamsError::UnknownSigningKey {
                kid: kid.to_owned(),
            });
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

    fn refetched_within(&self, now_unix: i64, window_secs: i64) -> bool {
        self.cache.read().is_ok_and(|guard| {
            guard
                .as_ref()
                .is_some_and(|cache| now_unix - cache.refreshed_at_unix < window_secs)
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
            .error_for_status()?
            .json()
            .await?;
        let jwks: Jwks = self
            .http
            .get(&config.jwks_uri)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(jwks.into_signing_keys())
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
    validation.set_required_spec_claims(&["exp", "iss", "aud"]);
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
