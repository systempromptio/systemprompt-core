//! Unit tests for `OAuthErrorCode` — RFC 6749 §5.2 wire strings and the default
//! HTTP status mapping the server uses when it doesn't override `with_status`.

use axum::http::StatusCode;
use systemprompt_api::routes::oauth::error::OAuthErrorCode;

#[test]
fn as_str_emits_canonical_rfc_wire_strings() {
    let pairs = [
        (OAuthErrorCode::InvalidRequest, "invalid_request"),
        (OAuthErrorCode::InvalidClient, "invalid_client"),
        (OAuthErrorCode::InvalidGrant, "invalid_grant"),
        (OAuthErrorCode::UnauthorizedClient, "unauthorized_client"),
        (OAuthErrorCode::UnsupportedGrantType, "unsupported_grant_type"),
        (OAuthErrorCode::InvalidScope, "invalid_scope"),
        (OAuthErrorCode::InvalidToken, "invalid_token"),
        (OAuthErrorCode::AccessDenied, "access_denied"),
        (OAuthErrorCode::ServerError, "server_error"),
        (OAuthErrorCode::TemporarilyUnavailable, "temporarily_unavailable"),
        (OAuthErrorCode::InvalidClientMetadata, "invalid_client_metadata"),
        (OAuthErrorCode::AuthenticationFailed, "authentication_failed"),
        (OAuthErrorCode::RegistrationFailed, "registration_failed"),
        (OAuthErrorCode::UsernameUnavailable, "username_unavailable"),
        (OAuthErrorCode::EmailExists, "email_exists"),
        (OAuthErrorCode::ExpiredChallenge, "expired_challenge"),
        (OAuthErrorCode::InvalidCredential, "invalid_credential"),
        (OAuthErrorCode::LinkFailed, "link_failed"),
        (OAuthErrorCode::InvalidTarget, "invalid_target"),
        (OAuthErrorCode::NotFound, "not_found"),
    ];
    for (code, wire) in pairs {
        assert_eq!(code.as_str(), wire);
    }
}

#[test]
fn default_status_400_for_request_shape_errors() {
    let codes = [
        OAuthErrorCode::InvalidRequest,
        OAuthErrorCode::InvalidGrant,
        OAuthErrorCode::UnauthorizedClient,
        OAuthErrorCode::UnsupportedGrantType,
        OAuthErrorCode::InvalidScope,
        OAuthErrorCode::InvalidClientMetadata,
        OAuthErrorCode::ExpiredChallenge,
        OAuthErrorCode::InvalidCredential,
        OAuthErrorCode::LinkFailed,
        OAuthErrorCode::InvalidTarget,
        OAuthErrorCode::RegistrationFailed,
    ];
    for c in codes {
        assert_eq!(c.default_status(), StatusCode::BAD_REQUEST, "{c:?}");
    }
}

#[test]
fn default_status_401_for_authentication_failures() {
    let codes = [
        OAuthErrorCode::InvalidClient,
        OAuthErrorCode::AccessDenied,
        OAuthErrorCode::AuthenticationFailed,
        OAuthErrorCode::InvalidToken,
    ];
    for c in codes {
        assert_eq!(c.default_status(), StatusCode::UNAUTHORIZED, "{c:?}");
    }
}

#[test]
fn default_status_409_for_conflicts() {
    assert_eq!(
        OAuthErrorCode::UsernameUnavailable.default_status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        OAuthErrorCode::EmailExists.default_status(),
        StatusCode::CONFLICT
    );
}

#[test]
fn default_status_misc_codes() {
    assert_eq!(
        OAuthErrorCode::NotFound.default_status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        OAuthErrorCode::ServerError.default_status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        OAuthErrorCode::TemporarilyUnavailable.default_status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[test]
fn copy_and_eq_semantics() {
    let a = OAuthErrorCode::InvalidGrant;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(a, OAuthErrorCode::InvalidRequest);
}

#[test]
fn debug_renders_variant_name() {
    let s = format!("{:?}", OAuthErrorCode::InvalidRequest);
    assert!(s.contains("InvalidRequest"), "debug={s}");
}
