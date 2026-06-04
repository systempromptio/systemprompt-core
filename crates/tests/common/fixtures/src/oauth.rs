//! OAuth client + PKCE test fixtures.
//!
//! [`seed_oauth_client`] inserts a confidential client (so the test can drive
//! `/token` with a known secret) plus a stable redirect URI. [`pkce_pair`]
//! produces a verifier/challenge pair using S256 — the only PKCE method
//! recommended for OAuth 2.1.

use anyhow::Result;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ClientId, UserId};
use systemprompt_oauth::repository::{ClientRepository, CreateClientParams};
use systemprompt_oauth::services::hash_client_secret;
use uuid::Uuid;

pub const TEST_CLIENT_SECRET: &str = "test-secret-must-be-long-enough-32chars";
pub const TEST_REDIRECT_URI: &str = "http://127.0.0.1/callback";

#[derive(Debug, Clone)]
pub struct OAuthClientFixture {
    pub client_id: ClientId,
    pub client_secret: String,
    pub redirect_uri: String,
}

pub async fn seed_oauth_client(pool: &DbPool, user_id: &UserId) -> Result<OAuthClientFixture> {
    let repo = ClientRepository::new(pool).map_err(|e| anyhow::anyhow!("client repo: {e}"))?;
    let client_id = ClientId::new(format!("test-client-{}", Uuid::new_v4().simple()));

    // Hash the secret with bcrypt — the same primitive the `/token` handler
    // calls via `verify_client_secret` when checking the supplied secret.
    let client_secret_hash = hash_client_secret(TEST_CLIENT_SECRET)
        .map_err(|e| anyhow::anyhow!("hash client secret: {e}"))?;

    repo.create(CreateClientParams {
        client_id: client_id.clone(),
        owner_user_id: user_id.clone(),
        client_secret_hash,
        client_name: "test-client".to_owned(),
        redirect_uris: vec![TEST_REDIRECT_URI.to_owned()],
        grant_types: Some(vec![
            "authorization_code".to_owned(),
            "refresh_token".to_owned(),
            "client_credentials".to_owned(),
        ]),
        response_types: Some(vec!["code".to_owned()]),
        scopes: vec!["openid".to_owned(), "profile".to_owned()],
        token_endpoint_auth_method: Some("client_secret_basic".to_owned()),
        application_type: "web".to_owned(),
        client_uri: None,
        logo_uri: None,
        contacts: None,
    })
    .await
    .map_err(|e| anyhow::anyhow!("create oauth client: {e}"))?;

    Ok(OAuthClientFixture {
        client_id,
        client_secret: TEST_CLIENT_SECRET.to_owned(),
        redirect_uri: TEST_REDIRECT_URI.to_owned(),
    })
}

#[derive(Debug, Clone)]
pub struct PkcePair {
    pub verifier: String,
    pub challenge: String,
    pub method: &'static str,
}

pub fn pkce_pair() -> PkcePair {
    // RFC 7636 §4.1 requires 43–128 chars from the unreserved set; a UUID
    // (without hyphens) twice over is 64 chars and satisfies that.
    let verifier = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
    PkcePair {
        verifier,
        challenge,
        method: "S256",
    }
}
