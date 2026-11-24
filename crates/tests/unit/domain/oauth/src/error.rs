//! Tests for the `OauthError` taxonomy.
//!
//! Exercises the `Display` impl across the security-meaningful
//! variants plus the `From` adapters that route foreign error types
//! into the enum.

use systemprompt_identifiers::{ClientId, UserId};
use systemprompt_oauth::{OauthError, OauthErrorKind, ValidationError};
use systemprompt_traits::FederatedRefusal;
use systemprompt_traits::{AuthProviderError, RepositoryError};

#[test]
fn user_provider_variant_keeps_context_and_source() {
    let err = OauthError::UserProvider {
        context: "loading the user",
        source: AuthProviderError::UserNotFound,
    };
    assert!(err.to_string().contains("loading the user"));
    assert!(std::error::Error::source(&err).is_some());
    assert_eq!(err.kind(), OauthErrorKind::ServerError);
}

#[test]
fn token_invalid_displays_inner_message() {
    let err = OauthError::TokenInvalid {
        reason: "malformed",
    };
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
    let err = OauthError::Expired {
        reason: "clock skew",
    };
    assert!(err.to_string().contains("expired"));
    assert!(err.to_string().contains("clock skew"));
}

#[test]
fn federated_link_refusal_displays_the_refusal() {
    let err = OauthError::FederatedLinkRefused {
        refusal: FederatedRefusal::NotProvisioned,
    };
    assert!(err.to_string().contains("invalid grant"));
    assert!(err.to_string().contains("provisioning is disabled"));
    assert_eq!(err.kind(), OauthErrorKind::InvalidGrant);
}

#[test]
fn invalid_client_displays_inner_message() {
    let err = OauthError::InvalidClient {
        reason: "not registered",
    };
    assert!(err.to_string().contains("invalid client"));
}

#[test]
fn client_not_found_displays_inner_message() {
    let err = OauthError::ClientNotFound {
        client_id: ClientId::new("client_xyz"),
    };
    assert!(err.to_string().contains("client_xyz"));
}

#[test]
fn webauthn_variants_display_correctly() {
    let wvf = OauthError::WebAuthnVerificationFailed {
        reason: "bad attestation",
    };
    assert!(wvf.to_string().contains("bad attestation"));

    let exp = OauthError::RegistrationStateExpired;
    assert!(exp.to_string().contains("expired"));
}

#[test]
fn user_variants_display_correctly() {
    let taken = OauthError::UsernameTaken {
        username: "alice".to_string(),
    };
    assert!(taken.to_string().contains("alice"));

    let registered = OauthError::EmailRegistered {
        email: "alice@example.com".to_string(),
    };
    assert!(registered.to_string().contains("alice@example.com"));

    let missing = OauthError::UserNotFound {
        user_id: UserId::new("user_999"),
    };
    assert!(missing.to_string().contains("user_999"));
}

#[test]
fn validation_displays_inner_message() {
    let err = OauthError::Validation(ValidationError::Rejected {
        reason: "bad scope",
    });
    assert!(err.to_string().contains("validation"));
    assert!(err.to_string().contains("bad scope"));
}

#[test]
fn internal_displays_inner_message() {
    let err = OauthError::Internal("bug");
    assert!(err.to_string().contains("internal"));
}

#[test]
fn bcrypt_error_converts_into_bcrypt_variant() {
    // bcrypt::hash with cost above the max emits BcryptError::CostNotAllowed.
    let bcrypt_err: bcrypt::BcryptError = bcrypt::hash("x", 100).unwrap_err();
    let err: OauthError = bcrypt_err.into();

    assert!(matches!(err, OauthError::Bcrypt(_)));
    assert_eq!(err.kind(), OauthErrorKind::ServerError);
}

#[test]
fn jsonwebtoken_error_converts_into_signing_failure() {
    let jwt_err = jsonwebtoken::decode_header("not-a-jwt").unwrap_err();
    let err: OauthError = jwt_err.into();

    assert!(matches!(err, OauthError::Signing(_)));
    assert_eq!(err.kind(), OauthErrorKind::ServerError);
}

#[test]
fn serde_json_error_converts_into_json_server_error() {
    let serde_err: serde_json::Error =
        serde_json::from_str::<serde_json::Value>("{ not json").unwrap_err();
    let err: OauthError = serde_err.into();

    assert!(matches!(err, OauthError::Json(_)));
    assert!(std::error::Error::source(&err).is_some());
    assert_eq!(err.kind(), OauthErrorKind::ServerError);
}

#[test]
fn oauth_error_implements_std_error() {
    let err = OauthError::Internal("x");
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

    let err: OauthError = AuthError::UnknownKid {
        kid: "kid-9".to_string(),
    }
    .into();
    assert!(matches!(&err, OauthError::TokenUnknownKid { kid } if kid == "kid-9"));
}

#[test]
fn security_auth_error_expired_signature_maps_to_expired() {
    use systemprompt_security::AuthError;

    let expired =
        jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::ExpiredSignature);
    let err: OauthError = AuthError::InvalidToken(expired).into();
    assert!(matches!(err, OauthError::Expired { .. }));

    let invalid =
        jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidSignature);
    let err: OauthError = AuthError::InvalidToken(invalid).into();
    assert!(matches!(err, OauthError::TokenRejected(_)));
    assert_eq!(err.kind(), OauthErrorKind::InvalidToken);

    let err: OauthError = AuthError::MissingAuthorization.into();
    assert!(matches!(err, OauthError::TokenRejected(_)));
}

#[test]
fn webauthn_error_converts_into_ceremony_failure() {
    let err: OauthError =
        webauthn_rs_via_service_error().expect_err("challenge mismatch is an error");
    assert!(matches!(err, OauthError::WebAuthnCeremony(_)));
    assert_eq!(err.kind(), OauthErrorKind::InvalidCredential);
}

fn webauthn_rs_via_service_error() -> Result<(), OauthError> {
    Err(webauthn_rs::prelude::WebauthnError::UserNotVerified.into())
}

#[test]
fn config_error_converts_into_config_variant() {
    let err: OauthError = systemprompt_models::errors::GlobalConfigError::NotInitialized.into();
    assert!(matches!(err, OauthError::Config(_)));
    assert!(err.to_string().contains("Config not initialized"));
}

#[test]
fn secrets_bootstrap_error_converts_into_secrets_variant() {
    let err: OauthError = systemprompt_config::SecretsBootstrapError::NoSecretsConfigured.into();
    assert!(matches!(err, OauthError::Secrets(_)));
    assert!(err.to_string().contains("No secrets configured"));
}

#[test]
fn setup_token_purpose_parse_error_converts_into_repository_decode() {
    let parse_err = "bogus"
        .parse::<systemprompt_oauth::repository::SetupTokenPurpose>()
        .expect_err("unknown purpose");
    let err: OauthError = parse_err.into();
    assert!(matches!(
        err,
        OauthError::Repository(RepositoryError::Decode { .. })
    ));
    assert_eq!(err.kind(), OauthErrorKind::ServerError);
}

#[test]
fn client_authentication_failures_classify_as_invalid_client() {
    for err in [
        OauthError::InvalidClient {
            reason: "Invalid client secret",
        },
        OauthError::ClientNotFound {
            client_id: ClientId::new("client_x"),
        },
    ] {
        assert_eq!(err.kind(), OauthErrorKind::InvalidClient);
        assert_eq!(err.kind().rfc_code(), "invalid_client");
    }
}

#[test]
fn repository_failure_is_a_server_error_not_an_auth_failure() {
    let err = OauthError::Repository(RepositoryError::database(std::io::Error::other(
        "pool closed",
    )));
    assert_eq!(err.kind(), OauthErrorKind::ServerError);
}

#[test]
fn unknown_account_and_missing_passkey_share_one_classification() {
    let err = OauthError::AuthenticationUnavailable;
    assert_eq!(err.kind(), OauthErrorKind::AuthenticationFailed);
}

#[test]
fn unique_violation_is_detected_from_the_repository_constraint() {
    let err = OauthError::Repository(RepositoryError::conflict("client", "c1", "stale"));
    assert!(!err.is_unique_violation());
}

#[test]
fn oauth_error_debug_includes_variant_name() {
    let err = OauthError::DcrClientNotFound {
        client_id: ClientId::new("client_abc"),
    };
    let debug = format!("{:?}", err);
    assert!(debug.contains("DcrClientNotFound"));
    assert!(debug.contains("client_abc"));
}
