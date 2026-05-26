//! Unit tests for the `From` impls on `OAuthHttpError` — the mapping of domain
//! errors (`OauthError`, `AuthProviderError`, `sqlx::Error`, `anyhow::Error`)
//! onto RFC 6749 codes.

use systemprompt_api::routes::oauth::{OAuthErrorCode, OAuthHttpError};
use systemprompt_oauth::OauthError;
use systemprompt_traits::auth::AuthProviderError;

#[test]
fn oauth_invalid_client_maps_to_invalid_client_code() {
    let e: OAuthHttpError = OauthError::InvalidClient("nope".into()).into();
    assert_eq!(e.code(), OAuthErrorCode::InvalidClient);
}

#[test]
fn oauth_client_not_found_maps_to_invalid_client() {
    let e: OAuthHttpError = OauthError::ClientNotFound("c1".into()).into();
    assert_eq!(e.code(), OAuthErrorCode::InvalidClient);
}

#[test]
fn oauth_invalid_grant_maps_to_invalid_grant() {
    for err in [
        OauthError::InvalidGrant("x".into()),
        OauthError::CodeNotFound("c".into()),
        OauthError::TokenNotFound("t".into()),
        OauthError::PkceMismatch("p".into()),
        OauthError::Expired("e".into()),
    ] {
        let e: OAuthHttpError = err.into();
        assert_eq!(e.code(), OAuthErrorCode::InvalidGrant);
    }
}

#[test]
fn oauth_validation_maps_to_invalid_request() {
    let e: OAuthHttpError = OauthError::Validation("bad".into()).into();
    assert_eq!(e.code(), OAuthErrorCode::InvalidRequest);
}

#[test]
fn oauth_unauthorized_maps_to_access_denied() {
    let e: OAuthHttpError = OauthError::Unauthorized("nope".into()).into();
    assert_eq!(e.code(), OAuthErrorCode::AccessDenied);
}

#[test]
fn oauth_username_taken_maps_to_username_unavailable() {
    let e: OAuthHttpError = OauthError::UsernameTaken("alice".into()).into();
    assert_eq!(e.code(), OAuthErrorCode::UsernameUnavailable);
}

#[test]
fn oauth_email_registered_maps_to_email_exists() {
    let e: OAuthHttpError = OauthError::EmailRegistered("a@b.com".into()).into();
    assert_eq!(e.code(), OAuthErrorCode::EmailExists);
}

#[test]
fn oauth_user_not_found_maps_to_not_found() {
    let e: OAuthHttpError = OauthError::UserNotFound("u".into()).into();
    assert_eq!(e.code(), OAuthErrorCode::NotFound);
}

#[test]
fn oauth_registration_state_expired_maps_to_expired_challenge() {
    let e: OAuthHttpError = OauthError::RegistrationStateExpired.into();
    assert_eq!(e.code(), OAuthErrorCode::ExpiredChallenge);
}

#[test]
fn oauth_webauthn_verification_failed_maps_to_invalid_credential() {
    let e: OAuthHttpError = OauthError::WebAuthnVerificationFailed("bad sig".into()).into();
    assert_eq!(e.code(), OAuthErrorCode::InvalidCredential);
}

#[test]
fn oauth_internal_kinds_map_to_server_error() {
    for err in [
        OauthError::WebAuthn("x".into()),
        OauthError::Session("s".into()),
        OauthError::Provider("p".into()),
        OauthError::Internal("i".into()),
        OauthError::TokenInvalid("t".into()),
        OauthError::TokenMissingKid,
    ] {
        let e: OAuthHttpError = err.into();
        assert_eq!(e.code(), OAuthErrorCode::ServerError);
    }
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

#[test]
fn anyhow_error_maps_to_server_error() {
    let e: OAuthHttpError = anyhow::anyhow!("boom").into();
    assert_eq!(e.code(), OAuthErrorCode::ServerError);
    assert!(e.description().contains("boom"));
}

#[test]
fn sqlx_row_not_found_maps_to_server_error() {
    let e: OAuthHttpError = sqlx::Error::RowNotFound.into();
    assert_eq!(e.code(), OAuthErrorCode::ServerError);
}
