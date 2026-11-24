//! Unit tests for the `From` impls on `OAuthHttpError` — the mapping of domain
//! errors (`OauthError`, `AuthProviderError`, `sqlx::Error`, `anyhow::Error`)
//! onto RFC 6749 codes.

use systemprompt_identifiers::{ClientId, UserId};
use systemprompt_identity_entry::http::oauth::{OAuthErrorCode, OAuthHttpError};
use systemprompt_oauth::{OauthError, ValidationError};
use systemprompt_traits::FederatedRefusal;
use systemprompt_traits::RepositoryError;
use systemprompt_traits::auth::AuthProviderError;

#[test]
fn oauth_invalid_client_maps_to_invalid_client_code() {
    let e: OAuthHttpError = OauthError::InvalidClient { reason: "nope" }.into();
    assert_eq!(e.code(), OAuthErrorCode::InvalidClient);
}

#[test]
fn oauth_client_not_found_maps_to_invalid_client() {
    let e: OAuthHttpError = OauthError::ClientNotFound {
        client_id: ClientId::new("c1"),
    }
    .into();
    assert_eq!(e.code(), OAuthErrorCode::InvalidClient);
}

#[test]
fn oauth_invalid_grant_maps_to_invalid_grant() {
    for err in [
        OauthError::FederatedLinkRefused {
            refusal: FederatedRefusal::EmailInUse,
        },
        OauthError::TokenInvalid { reason: "t" },
        OauthError::Expired { reason: "e" },
    ] {
        let e: OAuthHttpError = err.into();
        assert_eq!(e.code(), OAuthErrorCode::InvalidGrant);
    }
}

#[test]
fn oauth_validation_maps_to_invalid_request() {
    let e: OAuthHttpError =
        OauthError::Validation(ValidationError::Rejected { reason: "bad" }).into();
    assert_eq!(e.code(), OAuthErrorCode::InvalidRequest);
}

#[test]
fn oauth_service_account_interactive_maps_to_access_denied() {
    let e: OAuthHttpError = OauthError::ServiceAccountInteractive.into();
    assert_eq!(e.code(), OAuthErrorCode::AccessDenied);
}

#[test]
fn oauth_username_taken_maps_to_username_unavailable() {
    let e: OAuthHttpError = OauthError::UsernameTaken {
        username: "alice".into(),
    }
    .into();
    assert_eq!(e.code(), OAuthErrorCode::UsernameUnavailable);
}

#[test]
fn oauth_email_registered_maps_to_email_exists() {
    let e: OAuthHttpError = OauthError::EmailRegistered {
        email: "a@b.com".into(),
    }
    .into();
    assert_eq!(e.code(), OAuthErrorCode::EmailExists);
}

#[test]
fn oauth_user_not_found_maps_to_not_found() {
    let e: OAuthHttpError = OauthError::UserNotFound {
        user_id: UserId::new("u"),
    }
    .into();
    assert_eq!(e.code(), OAuthErrorCode::NotFound);
}

#[test]
fn oauth_registration_state_expired_maps_to_expired_challenge() {
    let e: OAuthHttpError = OauthError::RegistrationStateExpired.into();
    assert_eq!(e.code(), OAuthErrorCode::ExpiredChallenge);
}

#[test]
fn oauth_webauthn_verification_failed_maps_to_invalid_credential() {
    let e: OAuthHttpError = OauthError::WebAuthnVerificationFailed { reason: "bad sig" }.into();
    assert_eq!(e.code(), OAuthErrorCode::InvalidCredential);
}

#[test]
fn oauth_internal_kinds_map_to_server_error() {
    for err in [
        OauthError::Internal("i"),
        OauthError::WebAuthnConfig("not configured"),
        OauthError::Repository(RepositoryError::database(std::io::Error::other(
            "pool closed",
        ))),
        OauthError::InvalidTokenLifetime { seconds: 0 },
    ] {
        let e: OAuthHttpError = err.into();
        assert_eq!(e.code(), OAuthErrorCode::ServerError);
    }
}

#[test]
fn oauth_token_validation_failures_map_to_invalid_token() {
    for err in [
        OauthError::TokenMissingKid,
        OauthError::TokenUnknownKid { kid: "k".into() },
        OauthError::TokenAlgMismatch {
            got: "HS256".into(),
            expected: "RS256".into(),
        },
    ] {
        let e: OAuthHttpError = err.into();
        assert_eq!(e.code(), OAuthErrorCode::InvalidToken);
    }
}

#[test]
fn oauth_refresh_token_rejection_maps_to_invalid_grant() {
    let e: OAuthHttpError = OauthError::TokenInvalid {
        reason: "Invalid refresh token",
    }
    .into();
    assert_eq!(e.code(), OAuthErrorCode::InvalidGrant);
}

#[test]
fn wrong_client_secret_is_an_authentication_failure_not_a_server_error() {
    let e: OAuthHttpError = OauthError::InvalidClient {
        reason: "Invalid client secret",
    }
    .into();
    assert_eq!(e.code(), OAuthErrorCode::InvalidClient);
    assert_eq!(e.code().default_status(), http::StatusCode::UNAUTHORIZED);
}

#[test]
fn client_authentication_failures_share_one_description() {
    let wrong: OAuthHttpError = OauthError::InvalidClient {
        reason: "Invalid client secret",
    }
    .into();
    let unknown: OAuthHttpError = OauthError::ClientNotFound {
        client_id: ClientId::new("client_x"),
    }
    .into();
    assert_eq!(wrong.description(), unknown.description());
}

#[test]
fn webauthn_start_answers_identically_for_unknown_and_passkeyless_accounts() {
    let e: OAuthHttpError = OauthError::AuthenticationUnavailable.into();
    assert_eq!(e.code(), OAuthErrorCode::AuthenticationFailed);
    assert!(!e.description().contains('@'));
}

#[test]
fn auth_provider_invalid_credentials_maps_to_invalid_client() {
    let e: OAuthHttpError = AuthProviderError::InvalidCredentials.into();
    assert_eq!(e.code(), OAuthErrorCode::InvalidClient);
}

#[test]
fn auth_provider_invalid_token_maps_to_invalid_client() {
    let e: OAuthHttpError = AuthProviderError::InvalidToken.into();
    assert_eq!(e.code(), OAuthErrorCode::InvalidClient);
}

#[test]
fn auth_provider_user_not_found_maps_to_not_found() {
    let e: OAuthHttpError = AuthProviderError::UserNotFound.into();
    assert_eq!(e.code(), OAuthErrorCode::NotFound);
}

#[test]
fn auth_provider_token_expired_maps_to_invalid_grant() {
    let e: OAuthHttpError = AuthProviderError::TokenExpired.into();
    assert_eq!(e.code(), OAuthErrorCode::InvalidGrant);
}

#[test]
fn auth_provider_insufficient_permissions_maps_to_access_denied() {
    let e: OAuthHttpError = AuthProviderError::InsufficientPermissions.into();
    assert_eq!(e.code(), OAuthErrorCode::AccessDenied);
}
