//! Tests for `JwtValidationProviderImpl`: a verify-only provider over the
//! public verification key set, validating fixture-minted tokens.

use systemprompt_models::auth::JwtAudience;
use systemprompt_oauth::JwtValidationProviderImpl;
use systemprompt_security::keys::VerificationKeyring;
use systemprompt_traits::JwtValidationProvider;

fn keyring() -> VerificationKeyring {
    systemprompt_test_fixtures::test_signing_keys()
        .keyring()
        .clone()
}

#[test]
fn new_constructs_with_issuer_and_audiences() {
    let provider = JwtValidationProviderImpl::new(
        keyring(),
        "https://issuer.test".to_owned(),
        vec![JwtAudience::Api, JwtAudience::Web],
    );
    let debug = format!("{:?}", provider);

    assert!(debug.contains("JwtValidationProviderImpl"));
    assert!(debug.contains("https://issuer.test"));
}

#[test]
fn generate_secure_token_delegates_to_helper() {
    let provider =
        JwtValidationProviderImpl::new(keyring(), "issuer".to_owned(), vec![JwtAudience::Api]);

    let token = provider.generate_secure_token("auth");

    assert!(token.starts_with("auth_"));
    assert!(token.len() > "auth_".len());
}

#[test]
fn generate_secure_token_returns_unique_values() {
    let provider = JwtValidationProviderImpl::new(keyring(), "issuer".to_owned(), Vec::new());

    let a = provider.generate_secure_token("tok");
    let b = provider.generate_secure_token("tok");

    assert_ne!(a, b);
}

#[test]
fn validate_token_rejects_malformed_input() {
    let provider = JwtValidationProviderImpl::new(
        keyring(),
        "https://issuer.test".to_owned(),
        vec![JwtAudience::Api],
    );

    let err = provider
        .validate_token("not.a.valid.jwt")
        .expect_err("malformed token must be rejected");

    assert!(matches!(
        err,
        systemprompt_traits::JwtProviderError::InvalidToken
    ));
}

mod config_backed {
    use systemprompt_identifiers::{SessionId, UserId};
    use systemprompt_oauth::JwtValidationProviderImpl;
    use systemprompt_test_fixtures::{ensure_test_bootstrap, mint_admin_jwt};
    use systemprompt_traits::JwtValidationProvider;
    use uuid::Uuid;

    #[test]
    fn from_config_validates_fixture_minted_token() {
        ensure_test_bootstrap();
        let user_id = UserId::new(Uuid::new_v4().to_string());
        let token = mint_admin_jwt(&user_id, "prov@test.invalid", "https://issuer.test");

        let provider = JwtValidationProviderImpl::from_config(
            &systemprompt_test_fixtures::test_config(),
            super::keyring(),
        );
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

        let provider = JwtValidationProviderImpl::from_config(
            &systemprompt_test_fixtures::test_config(),
            super::keyring(),
        );
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

        let now = chrono::Utc::now();
        let claims = systemprompt_models::auth::JwtClaims {
            registered: systemprompt_models::auth::RegisteredClaims {
                sub: Uuid::new_v4().to_string(),
                iat: (now - chrono::Duration::hours(2)).timestamp(),
                exp: (now - chrono::Duration::hours(1)).timestamp(),
                nbf: Some((now - chrono::Duration::hours(2)).timestamp()),
                iss: "test".to_owned(),
                aud: systemprompt_models::auth::JwtAudience::standard(),
                jti: Uuid::new_v4().to_string(),
            },
            custom: systemprompt_models::auth::CustomClaims {
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
                purpose: None,
                interaction: None,
                browser_session: false,
            },
        };
        let keys = systemprompt_test_fixtures::test_signing_keys();
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some(keys.active_kid().to_owned());
        let token =
            jsonwebtoken::encode(&header, &claims, keys.encoding_key()).expect("encode expired");

        let provider = JwtValidationProviderImpl::from_config(
            &systemprompt_test_fixtures::test_config(),
            super::keyring(),
        );
        let err = provider
            .validate_token(&token)
            .expect_err("expired token must be rejected");
        assert!(matches!(
            err,
            systemprompt_traits::JwtProviderError::TokenExpired
        ));
    }

    #[test]
    fn validate_token_refuses_a_token_signed_outside_the_verification_set() {
        ensure_test_bootstrap();
        let user_id = UserId::new(Uuid::new_v4().to_string());
        let foreign = systemprompt_security::keys::SigningKeys::new(
            systemprompt_test_fixtures::keys::test_key(1),
            &[0; 32],
        )
        .expect("foreign keys");
        let params = systemprompt_security::jwt::AdminTokenParams {
            user_id: &user_id,
            session_id: &SessionId::generate(),
            email: "foreign@test.invalid",
            issuer: systemprompt_test_fixtures::FIXTURE_JWT_ISSUER,
            duration: chrono::Duration::hours(1),
            client_id: None,
        };
        let token = systemprompt_security::jwt::JwtService::generate_admin_token(&foreign, &params)
            .expect("mint with a foreign key");

        let provider = JwtValidationProviderImpl::from_config(
            &systemprompt_test_fixtures::test_config(),
            super::keyring(),
        );
        let err = provider
            .validate_token(token.as_str())
            .expect_err("a key outside the set must not verify");
        assert!(matches!(
            err,
            systemprompt_traits::JwtProviderError::InvalidToken
        ));
    }
}

#[test]
fn validate_token_rejects_empty_string() {
    let provider =
        JwtValidationProviderImpl::new(keyring(), "issuer".to_owned(), vec![JwtAudience::Api]);

    let err = provider
        .validate_token("")
        .expect_err("empty token must be rejected");
    let _ = err.to_string();
}
