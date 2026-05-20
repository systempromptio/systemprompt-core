//! Track B — distinct bridge JWT audience.
//!
//! Mirrors the validation contract enforced by the template's
//! `validate_bridge_jwt`: only `Bridge` is accepted; every other audience
//! (including the legacy `Api`) is rejected.

use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::Serialize;
use systemprompt_models::auth::JwtAudience;
use systemprompt_oauth::validate_jwt_token;

const TEST_SECRET: &str = "bridge_audience_test_secret_long_enough_padding";
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
        jti: "bridge-audience-test-jti",
        scope: "user",
        username: "bridge-tester",
        email: "bridge@example.com",
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
fn bridge_audience_token_accepted_by_bridge_validator() {
    let token = mint_token(JwtAudience::Bridge.as_str());
    let claims = validate_jwt_token(&token, TEST_ISSUER, &[JwtAudience::Bridge])
        .expect("Bridge-aud token must be accepted by validator allowing Bridge");
    assert!(claims.aud.contains(&JwtAudience::Bridge));
    assert_eq!(claims.sub, TEST_USER_UUID);
}

#[test]
fn legacy_api_audience_token_rejected() {
    let token = mint_token(JwtAudience::Api.as_str());
    let result = validate_jwt_token(&token, TEST_ISSUER, &[JwtAudience::Bridge]);
    assert!(
        result.is_err(),
        "Api-aud token must be rejected by Bridge-only validator (no grace fallback)"
    );
}

#[test]
fn non_bridge_non_api_audience_rejected() {
    let token = mint_token(JwtAudience::Mcp.as_str());
    let result = validate_jwt_token(&token, TEST_ISSUER, &[JwtAudience::Bridge]);
    assert!(
        result.is_err(),
        "Mcp-aud token must be rejected by Bridge validator"
    );
}
