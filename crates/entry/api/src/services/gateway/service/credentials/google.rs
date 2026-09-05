//! Google service-account credentials: RS256 assertion in, access token out.
//!
//! Implements the JWT-bearer profile (RFC 7523) that Google's token endpoint
//! accepts: sign a short assertion with the service account's private key,
//! POST it as `urn:ietf:params:oauth:grant-type:jwt-bearer`, receive an access
//! token valid for about an hour.
//!
//! Tokens are cached per secret name and reused until shortly before they
//! expire, because minting on every request would add a round trip to Google
//! in front of every round trip to the model. The skew is deliberate: a token
//! that expires in flight fails the *user's* request, so it is retired early
//! rather than used to the last second.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{OnceLock, PoisonError, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow, bail};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};

/// The assertion's own lifetime. Google caps this at one hour; it only has to
/// survive the exchange, so it is not the same thing as the token's lifetime.
const ASSERTION_TTL: Duration = Duration::from_secs(3600);

/// Retire a token this long before it actually expires, so one that would
/// expire mid-flight is replaced instead of being sent.
const EXPIRY_SKEW: Duration = Duration::from_secs(120);

/// Vertex AI accepts the broad cloud-platform scope; narrower scopes differ per
/// surface and would have to be configured per provider.
const SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";

#[derive(Debug, Deserialize)]
pub(super) struct ServiceAccountKey {
    pub(super) client_email: String,
    pub(super) private_key: String,
    #[serde(default = "default_token_uri")]
    pub(super) token_uri: String,
}

fn default_token_uri() -> String {
    "https://oauth2.googleapis.com/token".to_owned()
}

impl ServiceAccountKey {
    /// Recognise a Google service-account key by the `type` field the document
    /// declares about itself. Anything else — including any other JSON — is not
    /// one, and the caller treats it as a plain API key.
    pub(super) fn parse(secret: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(secret).ok()?;
        if value.get("type").and_then(serde_json::Value::as_str)? != "service_account" {
            return None;
        }
        serde_json::from_value(value).ok()
    }
}

#[derive(Debug, Serialize)]
struct Assertion<'a> {
    iss: &'a str,
    scope: &'a str,
    aud: &'a str,
    iat: u64,
    exp: u64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: u64,
}

#[derive(Debug, Clone)]
struct CachedToken {
    token: String,
    expires_at: SystemTime,
}

fn cache() -> &'static RwLock<HashMap<String, CachedToken>> {
    static CACHE: OnceLock<RwLock<HashMap<String, CachedToken>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// A cached token for `secret_name`, or a freshly minted one.
pub(super) async fn access_token(secret_name: &str, key: &ServiceAccountKey) -> Result<String> {
    if let Some(token) = cached(secret_name) {
        return Ok(token);
    }

    let response = exchange(key).await?;
    let ttl = if response.expires_in == 0 {
        ASSERTION_TTL
    } else {
        Duration::from_secs(response.expires_in)
    };

    cache()
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(
            secret_name.to_owned(),
            CachedToken {
                token: response.access_token.clone(),
                expires_at: SystemTime::now() + ttl,
            },
        );

    Ok(response.access_token)
}

fn cached(secret_name: &str) -> Option<String> {
    let guard = cache().read().unwrap_or_else(PoisonError::into_inner);
    let token = guard.get(secret_name).and_then(|entry| {
        (entry.expires_at > SystemTime::now() + EXPIRY_SKEW).then(|| entry.token.clone())
    });
    drop(guard);
    token
}

async fn exchange(key: &ServiceAccountKey) -> Result<TokenResponse> {
    let assertion = sign_assertion(key)?;

    let response = reqwest::Client::new()
        .post(&key.token_uri)
        .form(&[
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:jwt-bearer".to_owned(),
            ),
            ("assertion", assertion),
        ])
        .send()
        .await
        .map_err(|e| anyhow!("token endpoint {} unreachable: {e}", key.token_uri))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        // Why: Google answers a bad assertion with `invalid_grant` and no
        // detail, so the body is the only diagnostic an operator gets.
        bail!("token endpoint returned {status}: {}", body.trim());
    }

    serde_json::from_str(&body)
        .map_err(|e| anyhow!("token endpoint returned an unreadable body: {e}"))
}

fn sign_assertion(key: &ServiceAccountKey) -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| anyhow!("system clock is before the unix epoch: {e}"))?
        .as_secs();

    let claims = Assertion {
        iss: &key.client_email,
        scope: SCOPE,
        aud: &key.token_uri,
        iat: now,
        exp: now + ASSERTION_TTL.as_secs(),
    };

    let encoding = EncodingKey::from_rsa_pem(key.private_key.as_bytes())
        .map_err(|e| anyhow!("service-account private_key is not a valid RSA PEM: {e}"))?;

    jsonwebtoken::encode(&Header::new(Algorithm::RS256), &claims, &encoding)
        .map_err(|e| anyhow!("could not sign the assertion: {e}"))
}
