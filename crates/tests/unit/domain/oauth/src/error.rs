//! Tests for the `OauthError` taxonomy.
//!
//! Exercises the `Display` impl across the security-meaningful
//! variants plus the `From` adapters that route foreign error types
//! into the enum.

use systemprompt_oauth::OauthError;

#[test]
fn provider_variant_displays_inner_message() {
    let err = OauthError::Provider("github offline".to_string());
    assert!(err.to_string().contains("provider error"));
    assert!(err.to_string().contains("github offline"));
}

#[test]
fn token_invalid_displays_inner_message() {
    let err = OauthError::TokenInvalid("malformed".to_string());
    assert!(err.to_string().contains("token error"));
    assert!(err.to_string().contains("malformed"));
}

#[test]
fn token_alg_mismatch_carries_both_fields() {
    let err = OauthError::TokenAlgMismatch {
        got: "HS256".to_string(),
        expected: "RS256".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("HS256"));
    assert!(msg.contains("RS256"));
}

#[test]
fn token_missing_kid_has_static_message() {
    let err = OauthError::TokenMissingKid;
    assert!(err.to_string().contains("kid"));
}

#[test]
fn token_unknown_kid_carries_kid() {
    let err = OauthError::TokenUnknownKid {
        kid: "rotated-2024-12".to_string(),
    };
    assert!(err.to_string().contains("rotated-2024-12"));
}

#[test]
fn expired_displays_inner_message() {
    let err = OauthError::Expired("clock skew".to_string());
    assert!(err.to_string().contains("expired"));
    assert!(err.to_string().contains("clock skew"));
}

#[test]
fn pkce_mismatch_displays_inner_message() {
    let err = OauthError::PkceMismatch("S256 mismatch".to_string());
    assert!(err.to_string().contains("PKCE"));
    assert!(err.to_string().contains("S256 mismatch"));
}

#[test]
fn invalid_grant_displays_inner_message() {
    let err = OauthError::InvalidGrant("code consumed".to_string());
    assert!(err.to_string().contains("invalid grant"));
    assert!(err.to_string().contains("code consumed"));
}

#[test]
fn invalid_client_displays_inner_message() {
    let err = OauthError::InvalidClient("not registered".to_string());
    assert!(err.to_string().contains("invalid client"));
}

#[test]
fn client_not_found_displays_inner_message() {
    let err = OauthError::ClientNotFound("client_xyz".to_string());
    assert!(err.to_string().contains("client_xyz"));
}

#[test]
fn session_displays_inner_message() {
    let err = OauthError::Session("not found".to_string());
    assert!(err.to_string().contains("session"));
}

#[test]
fn webauthn_variants_display_correctly() {
    let we = OauthError::WebAuthn("ceremony failed".to_string());
    assert!(we.to_string().contains("webauthn"));

    let wvf = OauthError::WebAuthnVerificationFailed("bad attestation".to_string());
    assert!(wvf.to_string().contains("bad attestation"));

    let exp = OauthError::RegistrationStateExpired;
    assert!(exp.to_string().contains("expired"));
}

#[test]
fn user_variants_display_correctly() {
    let taken = OauthError::UsernameTaken("alice".to_string());
    assert!(taken.to_string().contains("alice"));

    let registered = OauthError::EmailRegistered("alice@example.com".to_string());
    assert!(registered.to_string().contains("alice@example.com"));

    let missing = OauthError::UserNotFound("user_999".to_string());
    assert!(missing.to_string().contains("user_999"));

    let user = OauthError::User("blocked".to_string());
    assert!(user.to_string().contains("blocked"));
}

#[test]
fn validation_displays_inner_message() {
    let err = OauthError::Validation("bad scope".to_string());
    assert!(err.to_string().contains("validation"));
    assert!(err.to_string().contains("bad scope"));
}

#[test]
fn unauthorized_displays_inner_message() {
    let err = OauthError::Unauthorized("not logged in".to_string());
    assert!(err.to_string().contains("unauthorized"));
}

#[test]
fn config_displays_inner_message() {
    let err = OauthError::Config("missing jwt_issuer".to_string());
    assert!(err.to_string().contains("config"));
    assert!(err.to_string().contains("missing jwt_issuer"));
}

#[test]
fn crypto_displays_inner_message() {
    let err = OauthError::Crypto("bcrypt failure".to_string());
    assert!(err.to_string().contains("crypto"));
}

#[test]
fn internal_displays_inner_message() {
    let err = OauthError::Internal("bug".to_string());
    assert!(err.to_string().contains("internal"));
}

#[test]
fn bcrypt_error_converts_into_crypto_variant() {
    // bcrypt::hash with cost above the max emits BcryptError::CostNotAllowed.
    let bcrypt_err: bcrypt::BcryptError = bcrypt::hash("x", 100).unwrap_err();
    let err: OauthError = bcrypt_err.into();

    assert!(matches!(err, OauthError::Crypto(_)));
}

#[test]
fn jsonwebtoken_error_converts_into_token_invalid() {
    let jwt_err = jsonwebtoken::decode_header("not-a-jwt").unwrap_err();
    let err: OauthError = jwt_err.into();

    assert!(matches!(err, OauthError::TokenInvalid(_)));
}

#[test]
fn serde_json_error_converts_into_validation() {
    let serde_err: serde_json::Error =
        serde_json::from_str::<serde_json::Value>("{ not json").unwrap_err();
    let err: OauthError = serde_err.into();

    match err {
        OauthError::Validation(msg) => assert!(msg.contains("json parse")),
        other => panic!("expected Validation, got {other:?}"),
    }
}

#[test]
fn oauth_error_implements_std_error() {
    let err = OauthError::Internal("x".to_string());
    let _boxed: Box<dyn std::error::Error> = Box::new(err);
}

#[test]
fn security_auth_error_maps_algorithm_and_kid_variants() {
    use systemprompt_security::AuthError;

    let err: OauthError = AuthError::UnsupportedAlgorithm {
        got: "HS256".to_string(),
    }
    .into();
    assert!(
        matches!(&err, OauthError::TokenAlgMismatch { got, expected } if got == "HS256" && expected == "RS256")
    );

    let err: OauthError = AuthError::MissingKid.into();
    assert!(matches!(err, OauthError::TokenMissingKid));

    let err: OauthError = AuthError::UnknownKid("kid-9".to_string()).into();
    assert!(matches!(&err, OauthError::TokenUnknownKid { kid } if kid == "kid-9"));
}

#[test]
fn security_auth_error_expired_signature_maps_to_expired() {
    use systemprompt_security::AuthError;

    let expired =
        jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::ExpiredSignature);
    let err: OauthError = AuthError::InvalidToken(expired).into();
    assert!(matches!(err, OauthError::Expired(_)));

    let invalid =
        jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidSignature);
    let err: OauthError = AuthError::InvalidToken(invalid).into();
    assert!(matches!(err, OauthError::TokenInvalid(_)));

    let err: OauthError = AuthError::MissingAuthorization.into();
    assert!(matches!(err, OauthError::TokenInvalid(_)));
}

#[test]
fn webauthn_error_converts_into_verification_failed() {
    let err: OauthError =
        webauthn_rs_via_service_error().expect_err("challenge mismatch is an error");
    assert!(matches!(err, OauthError::WebAuthnVerificationFailed(_)));
}

fn webauthn_rs_via_service_error() -> Result<(), OauthError> {
    Err(webauthn_rs::prelude::WebauthnError::UserNotVerified.into())
}

#[test]
fn config_error_converts_into_config_variant() {
    let err: OauthError = systemprompt_models::errors::ConfigError::NotInitialized.into();
    assert!(matches!(err, OauthError::Config(_)));
    assert!(err.to_string().contains("Config not initialized"));
}

#[test]
fn secrets_bootstrap_error_converts_into_config_variant() {
    let err: OauthError = systemprompt_config::SecretsBootstrapError::NotInitialized.into();
    assert!(matches!(err, OauthError::Config(_)));
    assert!(err.to_string().contains("Secrets not initialized"));
}

#[test]
fn setup_token_purpose_parse_error_converts_into_validation() {
    let parse_err = "bogus"
        .parse::<systemprompt_oauth::repository::SetupTokenPurpose>()
        .expect_err("unknown purpose");
    let err: OauthError = parse_err.into();
    assert!(matches!(err, OauthError::Validation(_)));
    assert!(err.to_string().contains("bogus"));
}

#[test]
fn oauth_error_debug_includes_variant_name() {
    let err = OauthError::TokenNotFound("tok".to_string());
    let debug = format!("{:?}", err);
    assert!(debug.contains("TokenNotFound"));

    let err2 = OauthError::CodeNotFound("auth_code_123".to_string());
    assert!(format!("{:?}", err2).contains("CodeNotFound"));
}
