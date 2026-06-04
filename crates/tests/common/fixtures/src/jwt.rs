//! Test-only signing key install and JWT minting helpers.
//!
//! [`install_test_signing_key`] is idempotent — the underlying authority cell
//! is process-wide, so the first caller wins. Concurrent test runs must share
//! the same minted key, which is fine because every test that consumes a JWT
//! resolves it via [`mint_admin_jwt`] / [`mint_bridge_jwt`].

use std::collections::BTreeMap;
use std::sync::OnceLock;

use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, Header};
use systemprompt_identifiers::{JwtToken, SessionId, UserId};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};
use systemprompt_security::jwt::{AdminTokenParams, JwtService};
use systemprompt_security::keys::authority::{active_kid, encoding_key, install_for_test};
use systemprompt_security::keys::RsaSigningKey;

static SIGNING_KEY: OnceLock<RsaSigningKey> = OnceLock::new();

pub fn install_test_signing_key() -> &'static RsaSigningKey {
    SIGNING_KEY.get_or_init(|| {
        let key = RsaSigningKey::generate_bits(2048).expect("rsa keygen");
        install_for_test(key.clone());
        key
    })
}

pub fn mint_admin_jwt(user_id: &UserId, email: &str, issuer: &str) -> JwtToken {
    install_test_signing_key();
    let session = SessionId::generate();
    let params = AdminTokenParams {
        user_id,
        session_id: &session,
        email,
        issuer,
        duration: Duration::hours(1),
        client_id: None,
    };
    JwtService::generate_admin_token(&params).expect("mint admin jwt")
}

pub fn mint_bridge_jwt(user_id: &UserId, email: &str, issuer: &str) -> JwtToken {
    install_test_signing_key();
    let session = SessionId::generate();
    let now = Utc::now();
    let expiry = now + Duration::hours(1);

    let claims = JwtClaims {
        sub: user_id.to_string(),
        iat: now.timestamp(),
        exp: expiry.timestamp(),
        nbf: Some(now.timestamp()),
        iss: issuer.to_owned(),
        aud: JwtAudience::standard(),
        jti: uuid::Uuid::new_v4().to_string(),
        scope: vec![Permission::User],
        username: email.to_owned(),
        email: email.to_owned(),
        user_type: UserType::User,
        roles: vec!["user".to_owned()],
        attributes: BTreeMap::new(),
        client_id: None,
        token_type: TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id: Some(session),
        rate_limit_tier: Some(RateLimitTier::User),
        plugin_id: None,
        act: None,
    };

    let kid = active_kid().expect("active kid present");
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_owned());
    let key = encoding_key().expect("encoding key present");
    let token = encode(&header, &claims, key).expect("encode bridge jwt");
    JwtToken::new(token)
}
