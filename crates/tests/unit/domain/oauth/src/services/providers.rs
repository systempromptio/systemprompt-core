//! Tests for `JwtValidationProviderImpl`: pure construction paths plus
//! config-backed validate/generate round-trips against the fixture
//! bootstrap and signing-key authority.

use systemprompt_models::auth::JwtAudience;
use systemprompt_oauth::JwtValidationProviderImpl;
use systemprompt_traits::JwtValidationProvider;

#[test]
fn new_constructs_with_issuer_and_audiences() {
    let provider = JwtValidationProviderImpl::new(
        "https://issuer.test".to_string(),
        vec![JwtAudience::Api, JwtAudience::Web],
    );
    let debug = format!("{:?}", provider);

    assert!(debug.contains("JwtValidationProviderImpl"));
    assert!(debug.contains("https://issuer.test"));
}

#[test]
fn new_accepts_empty_audiences() {
    let provider = JwtValidationProviderImpl::new("issuer".to_string(), Vec::new());

    let debug = format!("{:?}", provider);
    assert!(debug.contains("issuer"));
}

#[test]
fn generate_secure_token_delegates_to_helper() {
    let provider = JwtValidationProviderImpl::new("issuer".to_string(), vec![JwtAudience::Api]);

    let token = provider.generate_secure_token("auth");

    assert!(token.starts_with("auth_"));
    assert!(token.len() > "auth_".len());
}

#[test]
fn generate_secure_token_returns_unique_values() {
    let provider = JwtValidationProviderImpl::new("issuer".to_string(), Vec::new());

    let a = provider.generate_secure_token("tok");
    let b = provider.generate_secure_token("tok");

    assert_ne!(a, b);
}

#[test]
fn validate_token_rejects_malformed_input() {
    let provider =
        JwtValidationProviderImpl::new("https://issuer.test".to_string(), vec![JwtAudience::Api]);

    let err = provider
        .validate_token("not.a.valid.jwt")
        .expect_err("malformed token must be rejected");

    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("invalid") || msg.to_lowercase().contains("token"),
        "unexpected error message: {msg}"
    );
}

mod config_backed {
    use systemprompt_identifiers::{SessionId, UserId};
    use systemprompt_oauth::JwtValidationProviderImpl;
    use systemprompt_test_fixtures::{ensure_test_bootstrap, mint_admin_jwt};
    use systemprompt_traits::{GenerateTokenParams, JwtValidationProvider};
    use uuid::Uuid;

    #[test]
    fn from_config_validates_fixture_minted_token() {
        ensure_test_bootstrap();
        let user_id = UserId::new(Uuid::new_v4().to_string());
        let token = mint_admin_jwt(&user_id, "prov@test.invalid", "https://issuer.test");

        let provider = JwtValidationProviderImpl::from_config().expect("from_config");
        let claims = provider
            .validate_token(token.as_str())
            .expect("valid token");

        assert_eq!(claims.subject, user_id.as_str());
        assert!(claims.is_admin);
        assert!(claims.expires_at > claims.issued_at);
        assert!(claims.audiences.iter().any(|a| a == "api"));
    }

    #[test]
    fn validate_token_rejects_wrong_issuer_as_invalid() {
        ensure_test_bootstrap();
        let user_id = UserId::new(Uuid::new_v4().to_string());
        let token = mint_admin_jwt(&user_id, "prov2@test.invalid", "someone-else");

        let provider = JwtValidationProviderImpl::from_config().expect("from_config");
        let err = provider
            .validate_token(token.as_str())
            .expect_err("issuer mismatch");
        assert!(matches!(
            err,
            systemprompt_traits::JwtProviderError::InvalidToken
        ));
    }

    #[test]
    fn validate_token_maps_expired_signature_to_token_expired() {
        ensure_test_bootstrap();
        systemprompt_test_fixtures::install_test_signing_key();

        let now = chrono::Utc::now();
        let claims = systemprompt_models::auth::JwtClaims {
            sub: Uuid::new_v4().to_string(),
            iat: (now - chrono::Duration::hours(2)).timestamp(),
            exp: (now - chrono::Duration::hours(1)).timestamp(),
            nbf: Some((now - chrono::Duration::hours(2)).timestamp()),
            iss: "test".to_owned(),
            aud: systemprompt_models::auth::JwtAudience::standard(),
            jti: Uuid::new_v4().to_string(),
            scope: vec![systemprompt_models::auth::Permission::User],
            username: "expired-user".to_owned(),
            email: "expired@test.invalid".to_owned(),
            user_type: systemprompt_models::auth::UserType::User,
            roles: vec!["user".to_owned()],
            attributes: std::collections::BTreeMap::new(),
            client_id: None,
            token_type: systemprompt_models::auth::TokenType::Bearer,
            auth_time: (now - chrono::Duration::hours(2)).timestamp(),
            session_id: Some(SessionId::generate()),
            rate_limit_tier: None,
            plugin_id: None,
            act: None,
        };
        let kid = systemprompt_security::keys::authority::active_kid().expect("kid");
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some(kid.to_owned());
        let key = systemprompt_security::keys::authority::encoding_key().expect("key");
        let token = jsonwebtoken::encode(&header, &claims, key).expect("encode expired");

        let provider = JwtValidationProviderImpl::from_config().expect("from_config");
        let err = provider
            .validate_token(&token)
            .expect_err("expired token must be rejected");
        assert!(matches!(
            err,
            systemprompt_traits::JwtProviderError::TokenExpired
        ));
    }

    #[test]
    fn generate_token_roundtrips_through_validate() {
        ensure_test_bootstrap();
        systemprompt_test_fixtures::install_test_signing_key();
        let provider = JwtValidationProviderImpl::from_config().expect("from_config");
        let user_uuid = Uuid::new_v4();

        let token = provider
            .generate_token(GenerateTokenParams {
                user_id: UserId::new(user_uuid.to_string()),
                username: "prov-gen".to_owned(),
                user_type: "user".to_owned(),
                session_id: SessionId::generate(),
                permissions: vec!["user".to_owned(), "not-a-permission".to_owned()],
                audiences: vec!["api".to_owned(), "not-an-audience".to_owned()],
                expires_in_hours: Some(2),
            })
            .expect("generate");

        let claims = provider.validate_token(&token).expect("validate own token");
        assert_eq!(claims.subject, user_uuid.to_string());
        assert_eq!(claims.username, "prov-gen");
        assert!(claims.permissions.iter().any(|p| p == "user"));
        assert!(claims.audiences.iter().any(|a| a == "api"));
    }

    #[test]
    fn generate_token_defaults_audiences_when_empty() {
        ensure_test_bootstrap();
        systemprompt_test_fixtures::install_test_signing_key();
        let provider = JwtValidationProviderImpl::from_config().expect("from_config");

        let token = provider
            .generate_token(GenerateTokenParams {
                user_id: UserId::new(Uuid::new_v4().to_string()),
                username: "prov-default-aud".to_owned(),
                user_type: "user".to_owned(),
                session_id: SessionId::generate(),
                permissions: vec!["user".to_owned()],
                audiences: vec![],
                expires_in_hours: None,
            })
            .expect("generate");

        let claims = provider.validate_token(&token).expect("validate");
        assert!(claims.audiences.iter().any(|a| a == "api"));
    }

    #[test]
    fn generate_token_rejects_non_uuid_user_id() {
        ensure_test_bootstrap();
        systemprompt_test_fixtures::install_test_signing_key();
        let provider = JwtValidationProviderImpl::from_config().expect("from_config");

        let err = provider
            .generate_token(GenerateTokenParams {
                user_id: UserId::new("not-a-uuid"),
                username: "prov-bad".to_owned(),
                user_type: "user".to_owned(),
                session_id: SessionId::generate(),
                permissions: vec![],
                audiences: vec![],
                expires_in_hours: None,
            })
            .expect_err("non-uuid user id");
        assert!(err.to_string().contains("not a valid UUID"));
    }
}

#[test]
fn validate_token_rejects_empty_string() {
    let provider = JwtValidationProviderImpl::new("issuer".to_string(), vec![JwtAudience::Api]);

    let err = provider
        .validate_token("")
        .expect_err("empty token must be rejected");
    let _ = err.to_string();
}
