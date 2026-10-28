//! Test signing keys and JWT minting helpers.
//!
//! [`test_signing_keys`] builds the [`SigningKeys`] value every test hands to
//! the context or constructor under test. The RSA key is the committed fixture
//! from [`crate::keys`] and the manifest seed is the all-zero seed the fixture
//! secrets carry, so a value built here agrees with the keys an `AppContext`
//! resolves from the bootstrapped profile.

use std::collections::BTreeMap;

use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, Header};
use systemprompt_identifiers::{JwtToken, SessionId, UserId};
use systemprompt_models::auth::{
    CustomClaims, JwtAudience, JwtClaims, Permission, RateLimitTier, RegisteredClaims, TokenType,
    UserType,
};
use systemprompt_security::jwt::{AdminTokenParams, JwtService};
use systemprompt_security::keys::SigningKeys;

const TEST_MANIFEST_SEED: [u8; 32] = [0; 32];

pub fn test_signing_keys() -> SigningKeys {
    SigningKeys::new(
        crate::keys::test_key(crate::keys::AUTHORITY_KEY_INDEX),
        &TEST_MANIFEST_SEED,
    )
    .expect("build the fixture signing keys")
}

pub fn mint_admin_jwt(user_id: &UserId, email: &str, issuer: &str) -> JwtToken {
    let session = SessionId::generate();
    let params = AdminTokenParams {
        user_id,
        session_id: &session,
        email,
        issuer,
        duration: Duration::hours(1),
        client_id: None,
    };
    JwtService::generate_admin_token(&test_signing_keys(), &params).expect("mint admin jwt")
}

pub fn mint_bridge_jwt(user_id: &UserId, email: &str, issuer: &str) -> JwtToken {
    let session = SessionId::generate();
    let now = Utc::now();
    let expiry = now + Duration::hours(1);

    let claims = JwtClaims {
        registered: RegisteredClaims {
            sub: user_id.to_string(),
            iat: now.timestamp(),
            exp: expiry.timestamp(),
            nbf: Some(now.timestamp()),
            iss: issuer.to_owned(),
            aud: JwtAudience::standard(),
            jti: uuid::Uuid::new_v4().to_string(),
        },
        custom: CustomClaims {
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
            purpose: None,
            interaction: None,
            browser_session: false,
        },
    };

    let keys = test_signing_keys();
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(keys.active_kid().to_owned());
    let token = encode(&header, &claims, keys.encoding_key()).expect("encode bridge jwt");
    JwtToken::new(token)
}

pub fn test_token_authority(
    issuer: &str,
) -> std::sync::Arc<dyn systemprompt_models::auth::TokenAuthority> {
    std::sync::Arc::new(systemprompt_security::DeploymentTokenAuthority::new(
        test_signing_keys(),
        issuer,
    ))
}

pub fn test_oauth_signing() -> systemprompt_oauth::OAuthSigning {
    systemprompt_oauth::OAuthSigning {
        keys: test_signing_keys(),
        authority: test_token_authority(crate::app_context::FIXTURE_JWT_ISSUER),
    }
}
