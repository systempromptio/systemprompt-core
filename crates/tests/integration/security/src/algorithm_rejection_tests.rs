use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use systemprompt_identifiers::{ClientId, SessionId};
use systemprompt_models::auth::{
    CustomClaims, JwtAudience, JwtClaims, Permission, RateLimitTier, RegisteredClaims, TokenType,
    UserType,
};
use systemprompt_security::AuthError;
use systemprompt_security::jwt::{ValidationPolicy, extract_user_context_for};

fn sample_claims(iss: &str) -> JwtClaims {
    let now = Utc::now();
    JwtClaims {
        registered: RegisteredClaims {
            sub: "user-1".to_string(),
            iat: now.timestamp(),
            exp: (now + Duration::hours(1)).timestamp(),
            nbf: Some(now.timestamp()),
            iss: iss.to_string(),
            aud: vec![JwtAudience::Api],
            jti: "jti-1".to_string(),
        },
        custom: CustomClaims {
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
            purpose: None,
            interaction: None,
            browser_session: false,
            ui_session: false,
        },
    }
}

fn decode(token: &str) -> Result<systemprompt_models::auth::JwtUserContext, AuthError> {
    extract_user_context_for(
        systemprompt_test_fixtures::test_signing_keys().keyring(),
        token,
        &ValidationPolicy::issuer_scoped("integration-issuer", &[JwtAudience::Api]),
    )
}

#[tokio::test]
async fn hs256_token_is_rejected_with_unsupported_algorithm_error() {
    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some("any".to_string());
    let token = encode(
        &header,
        &sample_claims("integration-issuer"),
        &EncodingKey::from_secret(b"shared-secret-not-rs256"),
    )
    .expect("encode hs256");

    let err = decode(&token).expect_err("hs256 must be rejected");
    assert!(
        matches!(err, AuthError::UnsupportedAlgorithm { .. }),
        "expected UnsupportedAlgorithm, got {err:?}",
    );
}

#[tokio::test]
async fn rs256_token_without_kid_is_rejected_with_missing_kid_error() {
    let key = systemprompt_test_fixtures::next_test_key();
    let der = rsa::pkcs1::EncodeRsaPrivateKey::to_pkcs1_der(key.private_key()).expect("der");
    let enc = EncodingKey::from_rsa_der(der.as_bytes());
    let header = Header::new(Algorithm::RS256);
    let token = encode(&header, &sample_claims("integration-issuer"), &enc).expect("rs256 encode");

    let err = decode(&token).expect_err("missing kid must be rejected");
    assert!(
        matches!(err, AuthError::MissingKid),
        "expected MissingKid, got {err:?}",
    );
}

#[tokio::test]
async fn rs256_token_with_unknown_kid_is_rejected_with_unknown_kid_error() {
    let foreign = systemprompt_test_fixtures::next_test_key();
    let der = rsa::pkcs1::EncodeRsaPrivateKey::to_pkcs1_der(foreign.private_key()).expect("der");
    let enc = EncodingKey::from_rsa_der(der.as_bytes());
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(foreign.kid().to_string());
    let token = encode(&header, &sample_claims("integration-issuer"), &enc).expect("rs256 encode");

    let err = decode(&token).expect_err("foreign kid must be rejected");
    assert!(
        matches!(err, AuthError::UnknownKid(ref k) if k == foreign.kid()),
        "expected UnknownKid({}), got {err:?}",
        foreign.kid(),
    );
}
