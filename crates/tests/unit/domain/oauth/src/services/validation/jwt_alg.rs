use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use systemprompt_identifiers::{ClientId, SessionId};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};
use systemprompt_oauth::{OauthError, validate_jwt_token};

fn sample_claims() -> JwtClaims {
    let now = Utc::now();
    JwtClaims {
        sub: "user-1".to_string(),
        iat: now.timestamp(),
        exp: (now + Duration::hours(1)).timestamp(),
        nbf: Some(now.timestamp()),
        iss: "issuer".to_string(),
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

#[test]
fn hs256_token_yields_alg_mismatch() {
    let header = Header::new(Algorithm::HS256);
    let token = encode(
        &header,
        &sample_claims(),
        &EncodingKey::from_secret(b"shared-secret-not-rs256"),
    )
    .expect("encode hs256");

    let err = validate_jwt_token(&token, "issuer", &[JwtAudience::Api])
        .expect_err("HS256 must be rejected before signature check");
    match err {
        OauthError::TokenAlgMismatch { got, expected } => {
            assert_eq!(expected, "RS256");
            assert!(got.contains("HS256"), "expected got=HS256, got got=`{got}`");
        },
        other => panic!("expected TokenAlgMismatch, got {other:?}"),
    }
}
