//! Verifies the HS256 -> RS256 cutover contract:
//!
//! 1. tokens minted by the local authority carry the active `kid` and verify
//!    against the published JWKS;
//! 2. tokens signed with HS256 are rejected outright;
//! 3. tokens whose `kid` doesn't match the local authority are rejected;
//! 4. tokens signed by an unrelated RSA key are rejected (proxy for
//!    foreign-issuer JWKS lookup miss).

use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, decode_header, encode};
use rsa::pkcs1::EncodeRsaPrivateKey;
use systemprompt_identifiers::{ClientId, SessionId};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};
use systemprompt_security::keys::{RsaSigningKey, authority};

fn ensure_authority() -> &'static RsaSigningKey {
    systemprompt_test_fixtures::install_test_signing_key()
}

fn sample_claims(iss: &str) -> JwtClaims {
    let now = Utc::now();
    JwtClaims {
        sub: "user-1".to_string(),
        iat: now.timestamp(),
        exp: (now + Duration::hours(1)).timestamp(),
        nbf: Some(now.timestamp()),
        iss: iss.to_string(),
        aud: vec![JwtAudience::Api],
        jti: "jti-1".to_string(),
        scope: vec![Permission::User],
        username: "u".to_string(),
        email: "u@example.com".to_string(),
        user_type: UserType::User,
        roles: vec!["user".to_string()],
        attributes: std::collections::BTreeMap::new(),
        client_id: Some(ClientId::new("c")),
        token_type: TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id: Some(SessionId::new("s")),
        rate_limit_tier: Some(RateLimitTier::User),
        plugin_id: None,
        act: None,
    }
}

#[test]
fn locally_minted_token_carries_active_kid() {
    let key = ensure_authority();
    let kid = key.kid().to_string();

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.clone());
    let der = key
        .private_key()
        .to_pkcs1_der()
        .expect("encode private key");
    let enc = EncodingKey::from_rsa_der(der.as_bytes());
    let token = encode(&header, &sample_claims("issuer"), &enc).expect("encode token");

    let decoded_header = decode_header(&token).expect("decode header");
    assert_eq!(decoded_header.alg, Algorithm::RS256);
    assert_eq!(decoded_header.kid.as_deref(), Some(kid.as_str()));
}

#[test]
fn published_jwks_contains_active_kid() {
    let key = ensure_authority();
    let jwks = key.jwks();
    assert!(jwks.keys.iter().any(|jwk| jwk.kid == key.kid()));
}

#[test]
fn hs256_token_is_rejected_by_kid_lookup() {
    let _ = ensure_authority();
    let header = Header::new(Algorithm::HS256);
    let token = encode(
        &header,
        &sample_claims("issuer"),
        &EncodingKey::from_secret(b"shared-secret-not-rs256"),
    )
    .expect("encode hs256");

    let h = decode_header(&token).expect("hs256 token still decodes header");
    assert_eq!(h.alg, Algorithm::HS256);
    assert!(
        h.kid.is_none(),
        "HS256 token should not carry a `kid` matching the RS256 authority"
    );
}

#[test]
fn unknown_kid_is_rejected_by_authority() {
    let _ = ensure_authority();
    let lookup = authority::decoding_key_for_kid("bogus-kid").expect("authority callable");
    assert!(lookup.is_none(), "unknown kid must not resolve");
}

#[test]
fn foreign_key_is_not_accepted_via_local_lookup() {
    let local = ensure_authority();
    let foreign = systemprompt_test_fixtures::next_test_key();
    assert_ne!(
        local.kid(),
        foreign.kid(),
        "distinct RSA keys must produce distinct kids"
    );
    let lookup = authority::decoding_key_for_kid(foreign.kid()).expect("authority callable");
    assert!(
        lookup.is_none(),
        "tokens signed under a foreign issuer's key cannot be verified by the local authority"
    );
}

#[test]
fn active_decoding_key_verifies_locally_minted_token() {
    let key = ensure_authority();
    let decoding = authority::decoding_key().expect("decoding key available");

    let token = encode(
        &{
            let mut h = Header::new(Algorithm::RS256);
            h.kid = Some(key.kid().to_string());
            h
        },
        &sample_claims("https://issuer.test"),
        &{
            let der = key.private_key().to_pkcs1_der().expect("der");
            EncodingKey::from_rsa_der(der.as_bytes())
        },
    )
    .expect("mint token");

    let mut validation = jsonwebtoken::Validation::new(Algorithm::RS256);
    validation.validate_aud = false;
    let decoded = jsonwebtoken::decode::<JwtClaims>(&token, decoding, &validation)
        .expect("active decoding key must verify a token minted by the active signing key");
    assert_eq!(decoded.claims.sub, "user-1");
}

#[test]
fn install_accepts_the_installed_key_again() {
    let key = ensure_authority();
    authority::install(key.clone()).expect("reinstalling the same key is accepted");
    assert_eq!(authority::signing_key().expect("installed").kid(), key.kid());
}

#[test]
fn install_refuses_a_different_key() {
    let first = ensure_authority();
    let foreign = systemprompt_test_fixtures::next_test_key();

    let err = authority::install(foreign).expect_err("a second, different key must be refused");
    assert!(
        matches!(err, authority::TokenAuthorityError::ConflictingKey { .. }),
        "unexpected error: {err}"
    );
    assert_eq!(
        authority::signing_key().expect("still installed").kid(),
        first.kid(),
        "a refused install must not replace the installed authority"
    );
}
