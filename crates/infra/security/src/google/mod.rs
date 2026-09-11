//! Google service-account credentials: RS256 assertion in, access token out.
//!
//! Implements the JWT-bearer profile (RFC 7523) that Google's token endpoint
//! accepts: sign a short assertion with the service account's private key,
//! POST it as `urn:ietf:params:oauth:grant-type:jwt-bearer`, receive an access
//! token valid for about an hour.
//!
//! This module is now one *implementation* of the credential model in
//! [`crate::credential`] rather than a credential story of its own: it decides
//! how a Google key is signed and exchanged, and nothing else. Parsing,
//! caching, scoping and endpoint filling are generic and live there. A key
//! file names itself by its `type` field (`SERVICE_ACCOUNT_TYPE`);
//! `ServiceAccountKey::parse` keeps the name every existing caller uses but
//! the decision is made once, in [`ProviderCredential::parse`]. A token is
//! cached per stored secret, so two providers sharing one secret share one
//! token and two secrets never share one entry.
//!
//! Lives in the security crate rather than beside the gateway because
//! boot-time model discovery needs the same token with none of the gateway's
//! request machinery, and a credential minted in two places is two caches and
//! two ways to be wrong.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};

use crate::credential::cache::clamp_ttl;
use crate::credential::{CredentialError, ProviderCredential, http};

// Why: Google limits a service-account JWT assertion's lifetime to one hour.
const ASSERTION_TTL: Duration = Duration::from_secs(3600);

// Why: the assertion is rejected if its `iat` is in the future by even a
// second, and a host clock a little ahead of Google's is the ordinary case,
// not the exotic one. Back-dating costs nothing: the lifetime is measured
// from `iat`, so the token is not shortened, only started earlier.
const CLOCK_SKEW: Duration = Duration::from_secs(60);

const SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";

pub(crate) const SERVICE_ACCOUNT_TYPE: &str = "service_account";

#[derive(Clone, Deserialize)]
pub struct ServiceAccountKey {
    pub client_email: String,
    pub private_key: String,
    // Why: the project the key belongs to is the only project a token minted
    // from it can address, so it is where a `{project}` endpoint segment is
    // filled from — never from the catalog, which would name a tenant in a
    // file that ships with every image.
    pub project_id: String,
    #[serde(default = "default_token_uri")]
    pub token_uri: String,
}

// Why: the derived `Debug` would print `private_key`, and this type is held
// inside a `ProviderCredential` that the gateway logs the shape of. The only
// fields worth seeing are the ones that identify the key, not the one that is
// the key.
impl std::fmt::Debug for ServiceAccountKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceAccountKey")
            .field("client_email", &self.client_email)
            .field("project_id", &self.project_id)
            .field("token_uri", &self.token_uri)
            .field("private_key", &"<redacted>")
            .finish()
    }
}

fn default_token_uri() -> String {
    "https://oauth2.googleapis.com/token".to_owned()
}

impl ServiceAccountKey {
    pub fn parse(secret: &str) -> Result<Option<Self>, CredentialError> {
        match ProviderCredential::parse(secret)? {
            ProviderCredential::GoogleServiceAccount(key) => Ok(Some(*key)),
            ProviderCredential::ApiKey(_) => Ok(None),
        }
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
    expires_in: Option<u64>,
}

pub async fn access_token(
    cache_key: &str,
    key: &ServiceAccountKey,
) -> Result<String, CredentialError> {
    crate::credential::cache::token_for(cache_key, || async {
        let response = exchange(key).await?;
        Ok((response.access_token, clamp_ttl(response.expires_in)))
    })
    .await
}

async fn exchange(key: &ServiceAccountKey) -> Result<TokenResponse, CredentialError> {
    let assertion = sign_assertion(key)?;
    let form = [
        (
            "grant_type",
            "urn:ietf:params:oauth:grant-type:jwt-bearer".to_owned(),
        ),
        ("assertion", assertion),
    ];

    let body = http::post_form(&key.token_uri, &form).await?;
    serde_json::from_str(&body).map_err(|e| CredentialError::UnreadableBody(e.to_string()))
}

fn sign_assertion(key: &ServiceAccountKey) -> Result<String, CredentialError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| CredentialError::Clock(e.to_string()))?
        .as_secs()
        .saturating_sub(CLOCK_SKEW.as_secs());

    let claims = Assertion {
        iss: &key.client_email,
        scope: SCOPE,
        aud: &key.token_uri,
        iat: now,
        exp: now + ASSERTION_TTL.as_secs(),
    };

    let encoding = EncodingKey::from_rsa_pem(key.private_key.as_bytes())
        .map_err(|e| CredentialError::SigningKey(e.to_string()))?;

    jsonwebtoken::encode(&Header::new(Algorithm::RS256), &claims, &encoding)
        .map_err(|e| CredentialError::Sign(e.to_string()))
}
