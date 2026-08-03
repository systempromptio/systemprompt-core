//! `OAuthHttpError` — its constructors, the domain-error `From` impls, and the
//! `IntoResponse` rendering.
//!
//! This is the single place the OAuth surface decides what an RFC 6749 §5.2
//! error looks like on the wire, and nothing calls most of it: the constructors
//! are reached only through handlers, the `From` impls only through `?`, and
//! the redirect and `WWW-Authenticate` branches of `into_response` not at all.
//! A wrong code here is a client-visible protocol break.

use axum::body::to_bytes;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use systemprompt_api::routes::oauth::{OAuthErrorCode, OAuthHttpError};
use systemprompt_oauth::OauthError;
use systemprompt_traits::auth::AuthProviderError;

async fn rendered(err: OAuthHttpError) -> (StatusCode, serde_json::Value) {
    let resp = err.into_response();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("an error body is small and fully buffered");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn the_wire_body_is_the_rfc6749_error_pair() {
    let (status, body) = rendered(OAuthHttpError::invalid_request("missing state")).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"].as_str(), Some("invalid_request"), "{body}");
    assert_eq!(
        body["error_description"].as_str(),
        Some("missing state"),
        "{body}"
    );
}

#[test]
fn each_constructor_carries_its_own_code_and_default_status() {
    let cases = [
        (
            OAuthHttpError::invalid_request("d"),
            OAuthErrorCode::InvalidRequest,
        ),
        (
            OAuthHttpError::invalid_client("d"),
            OAuthErrorCode::InvalidClient,
        ),
        (
            OAuthHttpError::invalid_grant("d"),
            OAuthErrorCode::InvalidGrant,
        ),
        (
            OAuthHttpError::unauthorized_client("d"),
            OAuthErrorCode::UnauthorizedClient,
        ),
        (
            OAuthHttpError::unsupported_grant_type("d"),
            OAuthErrorCode::UnsupportedGrantType,
        ),
        (
            OAuthHttpError::invalid_scope("d"),
            OAuthErrorCode::InvalidScope,
        ),
        (
            OAuthHttpError::invalid_token("d"),
            OAuthErrorCode::InvalidToken,
        ),
        (
            OAuthHttpError::access_denied("d"),
            OAuthErrorCode::AccessDenied,
        ),
        (
            OAuthHttpError::server_error("d"),
            OAuthErrorCode::ServerError,
        ),
        (
            OAuthHttpError::invalid_client_metadata("d"),
            OAuthErrorCode::InvalidClientMetadata,
        ),
        (
            OAuthHttpError::authentication_failed("d"),
            OAuthErrorCode::AuthenticationFailed,
        ),
        (
            OAuthHttpError::registration_failed("d"),
            OAuthErrorCode::RegistrationFailed,
        ),
        (
            OAuthHttpError::username_unavailable("d"),
            OAuthErrorCode::UsernameUnavailable,
        ),
        (
            OAuthHttpError::email_exists("d"),
            OAuthErrorCode::EmailExists,
        ),
        (
            OAuthHttpError::expired_challenge("d"),
            OAuthErrorCode::ExpiredChallenge,
        ),
        (
            OAuthHttpError::invalid_credential("d"),
            OAuthErrorCode::InvalidCredential,
        ),
        (OAuthHttpError::link_failed("d"), OAuthErrorCode::LinkFailed),
        (
            OAuthHttpError::invalid_target("d"),
            OAuthErrorCode::InvalidTarget,
        ),
        (OAuthHttpError::not_found("d"), OAuthErrorCode::NotFound),
    ];

    for (err, expected) in cases {
        assert_eq!(err.code(), expected, "{}", err.description());
        assert_eq!(err.description(), "d");
    }
}

#[tokio::test]
async fn a_401_advertises_where_to_find_the_resource_metadata() {
    let resp = OAuthHttpError::invalid_token("expired").into_response();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let challenge = resp
        .headers()
        .get(header::WWW_AUTHENTICATE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        challenge.contains("resource_metadata=\"/.well-known/oauth-protected-resource\""),
        "RFC 9728 clients discover the authorization server from this header: {challenge}"
    );
}

#[tokio::test]
async fn a_non_401_carries_no_authenticate_challenge() {
    let resp = OAuthHttpError::invalid_request("bad").into_response();

    assert!(
        resp.headers().get(header::WWW_AUTHENTICATE).is_none(),
        "a 400 is not an authentication prompt"
    );
}

#[test]
fn an_explicit_status_overrides_the_code_default() {
    let err =
        OAuthHttpError::invalid_client_metadata("duplicate").with_status(StatusCode::CONFLICT);

    assert_eq!(err.into_response().status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn a_redirecting_error_goes_back_to_the_client_with_code_and_state() {
    let err = OAuthHttpError::access_denied("user refused")
        .with_redirect("http://127.0.0.1/callback", Some("opaque state".to_owned()));

    let resp = err.into_response();

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        location.starts_with("http://127.0.0.1/callback?"),
        "{location}"
    );
    assert!(location.contains("error=access_denied"), "{location}");
    assert!(
        location.contains("state=opaque%20state"),
        "the client's state must be returned percent-encoded: {location}"
    );
}

#[tokio::test]
async fn a_redirecting_error_without_state_omits_the_parameter() {
    let err =
        OAuthHttpError::invalid_scope("nope").with_redirect("http://127.0.0.1/callback", None);

    let resp = err.into_response();
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    assert!(location.contains("error=invalid_scope"), "{location}");
    assert!(
        !location.contains("state="),
        "a request that sent no state must get none back: {location}"
    );
}

#[test]
fn oauth_domain_errors_map_onto_their_protocol_codes() {
    let cases: Vec<(OauthError, OAuthErrorCode)> = vec![
        (
            OauthError::InvalidClient("c".to_owned()),
            OAuthErrorCode::InvalidClient,
        ),
        (
            OauthError::ClientNotFound("c".to_owned()),
            OAuthErrorCode::InvalidClient,
        ),
        (
            OauthError::InvalidGrant("g".to_owned()),
            OAuthErrorCode::InvalidGrant,
        ),
        (
            OauthError::CodeNotFound("c".to_owned()),
            OAuthErrorCode::InvalidGrant,
        ),
        (
            OauthError::PkceMismatch("p".to_owned()),
            OAuthErrorCode::InvalidGrant,
        ),
        (
            OauthError::Expired("e".to_owned()),
            OAuthErrorCode::InvalidGrant,
        ),
        (
            OauthError::Validation("v".to_owned()),
            OAuthErrorCode::InvalidRequest,
        ),
        (
            OauthError::Unauthorized("u".to_owned()),
            OAuthErrorCode::AccessDenied,
        ),
        (
            OauthError::UsernameTaken("n".to_owned()),
            OAuthErrorCode::UsernameUnavailable,
        ),
        (
            OauthError::EmailRegistered("e".to_owned()),
            OAuthErrorCode::EmailExists,
        ),
        (
            OauthError::UserNotFound("u".to_owned()),
            OAuthErrorCode::NotFound,
        ),
        (
            OauthError::RegistrationStateExpired,
            OAuthErrorCode::ExpiredChallenge,
        ),
        (
            OauthError::WebAuthnVerificationFailed("w".to_owned()),
            OAuthErrorCode::InvalidCredential,
        ),
        (
            OauthError::Internal("i".to_owned()),
            OAuthErrorCode::ServerError,
        ),
        (OauthError::TokenMissingKid, OAuthErrorCode::ServerError),
    ];

    for (err, expected) in cases {
        let rendered = err.to_string();
        let http: OAuthHttpError = err.into();
        assert_eq!(http.code(), expected, "{rendered}");
    }
}

#[test]
fn a_taken_username_does_not_echo_the_internal_error_text() {
    // The description is client-facing; leaking the repository's wording here
    // would expose which account names exist beyond the code itself.
    let http: OAuthHttpError = OauthError::UsernameTaken("dbuser42".to_owned()).into();

    assert_eq!(http.code(), OAuthErrorCode::UsernameUnavailable);
    assert!(
        !http.description().contains("dbuser42"),
        "{}",
        http.description()
    );
}

#[test]
fn auth_provider_errors_map_onto_their_protocol_codes() {
    let cases = [
        (
            AuthProviderError::InvalidCredentials,
            OAuthErrorCode::InvalidClient,
        ),
        (
            AuthProviderError::InvalidToken,
            OAuthErrorCode::InvalidClient,
        ),
        (AuthProviderError::UserNotFound, OAuthErrorCode::NotFound),
        (
            AuthProviderError::TokenExpired,
            OAuthErrorCode::InvalidGrant,
        ),
        (
            AuthProviderError::InsufficientPermissions,
            OAuthErrorCode::AccessDenied,
        ),
    ];

    for (err, expected) in cases {
        let rendered = err.to_string();
        let http: OAuthHttpError = err.into();
        assert_eq!(http.code(), expected, "{rendered}");
    }
}

#[test]
fn an_anyhow_error_collapses_to_a_server_error() {
    let http: OAuthHttpError = anyhow::anyhow!("something deep failed").into();

    assert_eq!(http.code(), OAuthErrorCode::ServerError);
    assert!(http.description().contains("something deep failed"));
}

#[test]
fn a_plain_database_error_is_a_server_error_not_a_client_one() {
    let http: OAuthHttpError = sqlx::Error::RowNotFound.into();

    assert_eq!(
        http.code(),
        OAuthErrorCode::ServerError,
        "a row-not-found is our problem, not the caller's"
    );
}
