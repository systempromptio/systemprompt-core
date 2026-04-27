//! Track B — distinct cowork JWT audience with 30-day Api grace window.
//!
//! Mirrors the validation contract enforced by the template's
//! `validate_cowork_jwt`: PRIMARY = `Cowork`, GRACE = `Api` (until 2026-05-27),
//! everything else rejected.

use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::Serialize;
use systemprompt_models::auth::JwtAudience;
use systemprompt_oauth::validate_jwt_token;

const TEST_SECRET: &str = "cowork_audience_test_secret_long_enough_padding";
const TEST_ISSUER: &str = "https://test.systemprompt.io";
const TEST_USER_UUID: &str = "550e8400-e29b-41d4-a716-446655440099";

#[derive(Serialize)]
struct RawClaims<'a> {
    sub: &'a str,
    iat: i64,
    exp: i64,
    iss: &'a str,
    aud: Vec<&'a str>,
    jti: &'a str,
    scope: &'a str,
    username: &'a str,
    email: &'a str,
    user_type: &'a str,
    token_type: &'a str,
    auth_time: i64,
}

fn mint_token(audience: &str) -> String {
    let now = Utc::now().timestamp();
    let claims = RawClaims {
        sub: TEST_USER_UUID,
        iat: now,
        exp: now + 3600,
        iss: TEST_ISSUER,
        aud: vec![audience],
        jti: "cowork-audience-test-jti",
        scope: "user",
        username: "cowork-tester",
        email: "cowork@example.com",
        user_type: "user",
        token_type: "Bearer",
        auth_time: now,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
    )
    .expect("encode test token")
}

#[test]
fn cowork_audience_token_accepted_by_cowork_validator() {
    let token = mint_token(JwtAudience::Cowork.as_str());
    let claims = validate_jwt_token(&token, TEST_SECRET, TEST_ISSUER, &[JwtAudience::Cowork])
        .expect("Cowork-aud token must be accepted by validator allowing Cowork");
    assert!(claims.aud.contains(&JwtAudience::Cowork));
    assert_eq!(claims.sub, TEST_USER_UUID);
}

#[test]
fn legacy_api_audience_token_accepted_during_grace_window() {
    let token = mint_token(JwtAudience::Api.as_str());
    let primary = validate_jwt_token(&token, TEST_SECRET, TEST_ISSUER, &[JwtAudience::Cowork]);
    assert!(
        primary.is_err(),
        "Cowork-only validator must reject Api-aud token (forces fallback path)"
    );
    let fallback = validate_jwt_token(&token, TEST_SECRET, TEST_ISSUER, &[JwtAudience::Api])
        .expect("Api-aud token must still validate during grace window (until 2026-05-27)");
    assert!(fallback.aud.contains(&JwtAudience::Api));
}

#[test]
fn non_cowork_non_api_audience_rejected() {
    let token = mint_token(JwtAudience::Mcp.as_str());
    let primary = validate_jwt_token(&token, TEST_SECRET, TEST_ISSUER, &[JwtAudience::Cowork]);
    assert!(primary.is_err(), "Mcp-aud token must be rejected by Cowork validator");
    let fallback = validate_jwt_token(&token, TEST_SECRET, TEST_ISSUER, &[JwtAudience::Api]);
    assert!(
        fallback.is_err(),
        "Mcp-aud token must be rejected by Api fallback validator"
    );
}
