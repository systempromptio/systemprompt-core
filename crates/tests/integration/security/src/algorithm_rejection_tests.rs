use std::sync::Once;

use http::HeaderMap;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use systemprompt_identifiers::{ClientId, SessionId};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};
use systemprompt_security::keys::{RsaSigningKey, authority};
use systemprompt_security::{AuthError, AuthMode, AuthValidationService};

static INSTALL: Once = Once::new();

fn ensure_authority() {
    INSTALL.call_once(|| {
        let key = RsaSigningKey::generate_bits(2048).expect("rsa");
        authority::install_for_test(key);
    });
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
        department: None,
        client_id: Some(ClientId::new("c")),
        token_type: TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id: Some(SessionId::new("s")),
        rate_limit_tier: Some(RateLimitTier::User),
        plugin_id: None,
        act: None,
    }
}

fn auth_header(token: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        "authorization",
        format!("Bearer {token}").parse().expect("hv"),
    );
    h
}

#[tokio::test]
async fn hs256_token_is_rejected_with_unsupported_algorithm_error() {
    ensure_authority();
    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some("any".to_string());
    let token = encode(
        &header,
        &sample_claims("integration-issuer"),
        &EncodingKey::from_secret(b"shared-secret-not-rs256"),
    )
    .expect("encode hs256");

    let svc = AuthValidationService::new(
        "integration-issuer".to_string(),
        vec![JwtAudience::Api],
    );
    let err = svc
        .validate_request(&auth_header(&token), AuthMode::Required)
        .expect_err("hs256 must be rejected");
    assert!(
        matches!(err, AuthError::UnsupportedAlgorithm),
        "expected UnsupportedAlgorithm, got {err:?}",
    );
}

#[tokio::test]
async fn rs256_token_without_kid_is_rejected_with_missing_kid_error() {
    ensure_authority();
    let key = RsaSigningKey::generate_bits(2048).expect("rsa");
    let der = rsa::pkcs1::EncodeRsaPrivateKey::to_pkcs1_der(key.private_key())
        .expect("der");
    let enc = EncodingKey::from_rsa_der(der.as_bytes());
    let header = Header::new(Algorithm::RS256);
    let token =
        encode(&header, &sample_claims("integration-issuer"), &enc).expect("rs256 encode");

    let svc = AuthValidationService::new(
        "integration-issuer".to_string(),
        vec![JwtAudience::Api],
    );
    let err = svc
        .validate_request(&auth_header(&token), AuthMode::Required)
        .expect_err("missing kid must be rejected");
    assert!(
        matches!(err, AuthError::MissingKid),
        "expected MissingKid, got {err:?}",
    );
}

#[tokio::test]
async fn rs256_token_with_unknown_kid_is_rejected_with_unknown_kid_error() {
    ensure_authority();
    let foreign = RsaSigningKey::generate_bits(2048).expect("rsa");
    let der = rsa::pkcs1::EncodeRsaPrivateKey::to_pkcs1_der(foreign.private_key())
        .expect("der");
    let enc = EncodingKey::from_rsa_der(der.as_bytes());
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(foreign.kid().to_string());
    let token =
        encode(&header, &sample_claims("integration-issuer"), &enc).expect("rs256 encode");

    let svc = AuthValidationService::new(
        "integration-issuer".to_string(),
        vec![JwtAudience::Api],
    );
    let err = svc
        .validate_request(&auth_header(&token), AuthMode::Required)
        .expect_err("foreign kid must be rejected");
    assert!(
        matches!(err, AuthError::UnknownKid(ref k) if k == foreign.kid()),
        "expected UnknownKid({}), got {err:?}",
        foreign.kid(),
    );
}
