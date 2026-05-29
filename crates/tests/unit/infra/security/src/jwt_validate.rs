use systemprompt_security::jwt::{ValidationPolicy, JWT_LEEWAY_SECONDS};
use systemprompt_security::AuthError;
use systemprompt_test_fixtures::install_test_signing_key;

#[test]
fn session_context_policy_has_exp_nbf() {
    let p = ValidationPolicy::session_context();
    assert!(p.validate_exp);
    assert!(p.validate_nbf);
    assert_eq!(p.leeway_seconds, JWT_LEEWAY_SECONDS);
    assert!(p.issuer.is_none());
    assert!(p.audiences.is_empty());
}

#[test]
fn issuer_scoped_policy_pins_issuer_and_audiences() {
    use systemprompt_models::auth::JwtAudience;
    let auds = [JwtAudience::Web];
    let p = ValidationPolicy::issuer_scoped("my.issuer", &auds);
    assert!(p.validate_exp);
    assert!(p.validate_nbf);
    assert_eq!(p.issuer, Some("my.issuer"));
    assert_eq!(p.audiences.len(), 1);
}

#[test]
fn decode_rejects_non_rs256_token() {
    install_test_signing_key();
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct MinClaims {
        sub: String,
        exp: i64,
    }

    let secret = b"super-secret-key-that-is-quite-long-enough";
    let claims = MinClaims {
        sub: "u1".to_owned(),
        exp: 9999999999,
    };
    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some("kid1".to_owned());
    let token = encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap();

    let policy = ValidationPolicy::session_context();
    let err = systemprompt_security::jwt::decode_rs256_claims(&token, &policy).unwrap_err();
    assert!(
        matches!(err, AuthError::UnsupportedAlgorithm { .. }),
        "expected UnsupportedAlgorithm, got {err:?}"
    );
}

#[test]
fn decode_rejects_missing_kid() {
    install_test_signing_key();
    use chrono::{Duration, Utc};
    use jsonwebtoken::{encode, Algorithm, Header};
    use systemprompt_security::keys::authority;

    let now = Utc::now();
    let claims = systemprompt_models::auth::JwtClaims {
        sub: "u1".to_owned(),
        iat: now.timestamp(),
        exp: (now + Duration::hours(1)).timestamp(),
        nbf: None,
        iss: "test".to_owned(),
        aud: systemprompt_models::auth::JwtAudience::standard(),
        jti: "jti1".to_owned(),
        scope: vec![],
        username: "u@test.com".to_owned(),
        email: "u@test.com".to_owned(),
        user_type: systemprompt_models::auth::UserType::User,
        roles: vec![],
        attributes: Default::default(),
        client_id: None,
        token_type: systemprompt_models::auth::TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id: None,
        rate_limit_tier: None,
        plugin_id: None,
        act: None,
    };

    let mut header = Header::new(Algorithm::RS256);
    header.kid = None;
    let key = authority::encoding_key().expect("encoding key");
    let token = encode(&header, &claims, key).unwrap();

    let policy = ValidationPolicy::session_context();
    let err = systemprompt_security::jwt::decode_rs256_claims(&token, &policy).unwrap_err();
    assert!(
        matches!(err, AuthError::MissingKid),
        "expected MissingKid, got {err:?}"
    );
}

#[test]
fn decode_rejects_unknown_kid() {
    install_test_signing_key();
    use chrono::{Duration, Utc};
    use jsonwebtoken::{encode, Algorithm, Header};
    use systemprompt_security::keys::authority;

    let now = Utc::now();
    let claims = systemprompt_models::auth::JwtClaims {
        sub: "u1".to_owned(),
        iat: now.timestamp(),
        exp: (now + Duration::hours(1)).timestamp(),
        nbf: None,
        iss: "test".to_owned(),
        aud: systemprompt_models::auth::JwtAudience::standard(),
        jti: "jti1".to_owned(),
        scope: vec![],
        username: "u@test.com".to_owned(),
        email: "u@test.com".to_owned(),
        user_type: systemprompt_models::auth::UserType::User,
        roles: vec![],
        attributes: Default::default(),
        client_id: None,
        token_type: systemprompt_models::auth::TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id: None,
        rate_limit_tier: None,
        plugin_id: None,
        act: None,
    };

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("unknown-kid-xyz".to_owned());
    let key = authority::encoding_key().expect("encoding key");
    let token = encode(&header, &claims, key).unwrap();

    let policy = ValidationPolicy::session_context();
    let err = systemprompt_security::jwt::decode_rs256_claims(&token, &policy).unwrap_err();
    assert!(
        matches!(err, AuthError::UnknownKid(_)),
        "expected UnknownKid, got {err:?}"
    );
}

#[test]
fn leeway_constant_is_thirty_seconds() {
    assert_eq!(JWT_LEEWAY_SECONDS, 30);
}
