//! Unit tests for `OAuthHttpError` constructors and `IntoResponse`.

use axum::body::to_bytes;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use serde_json::Value;
use systemprompt_api::routes::oauth::{OAuthErrorCode, OAuthHttpError};

async fn body_string(
    resp: axum::response::Response,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await.expect("body");
    (
        status,
        headers,
        String::from_utf8(bytes.to_vec()).unwrap_or_default(),
    )
}

#[test]
fn new_sets_status_from_code_default() {
    let e = OAuthHttpError::new(OAuthErrorCode::InvalidRequest, "x");
    assert_eq!(e.code(), OAuthErrorCode::InvalidRequest);
    assert_eq!(e.description(), "x");
}

#[test]
fn helper_constructors_match_code() {
    macro_rules! check {
        ($ctor:ident, $code:expr) => {{
            let e = OAuthHttpError::$ctor("d");
            assert_eq!(e.code(), $code);
            assert_eq!(e.description(), "d");
        }};
    }
    check!(invalid_request, OAuthErrorCode::InvalidRequest);
    check!(invalid_client, OAuthErrorCode::InvalidClient);
    check!(invalid_grant, OAuthErrorCode::InvalidGrant);
    check!(unauthorized_client, OAuthErrorCode::UnauthorizedClient);
    check!(unsupported_grant_type, OAuthErrorCode::UnsupportedGrantType);
    check!(invalid_scope, OAuthErrorCode::InvalidScope);
    check!(invalid_token, OAuthErrorCode::InvalidToken);
    check!(access_denied, OAuthErrorCode::AccessDenied);
    check!(server_error, OAuthErrorCode::ServerError);
    check!(
        invalid_client_metadata,
        OAuthErrorCode::InvalidClientMetadata
    );
    check!(authentication_failed, OAuthErrorCode::AuthenticationFailed);
    check!(registration_failed, OAuthErrorCode::RegistrationFailed);
    check!(username_unavailable, OAuthErrorCode::UsernameUnavailable);
    check!(email_exists, OAuthErrorCode::EmailExists);
    check!(expired_challenge, OAuthErrorCode::ExpiredChallenge);
    check!(invalid_credential, OAuthErrorCode::InvalidCredential);
    check!(link_failed, OAuthErrorCode::LinkFailed);
    check!(invalid_target, OAuthErrorCode::InvalidTarget);
    check!(not_found, OAuthErrorCode::NotFound);
}

#[test]
fn with_status_overrides_default() {
    let e = OAuthHttpError::invalid_request("d").with_status(StatusCode::IM_A_TEAPOT);
    let resp = e.into_response();
    assert_eq!(resp.status(), StatusCode::IM_A_TEAPOT);
}

#[tokio::test]
async fn into_response_emits_rfc_6749_json_body() {
    let e = OAuthHttpError::invalid_grant("token expired");
    let resp = e.into_response();
    let (status, _, body) = body_string(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let json: Value = serde_json::from_str(&body).expect("json");
    assert_eq!(json["error"], "invalid_grant");
    assert_eq!(json["error_description"], "token expired");
}

#[tokio::test]
async fn unauthorized_response_includes_www_authenticate_header() {
    let e = OAuthHttpError::invalid_client("nope");
    let resp = e.into_response();
    let (status, headers, _) = body_string(resp).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let v = headers
        .get(header::WWW_AUTHENTICATE)
        .expect("WWW-Authenticate set on 401")
        .to_str()
        .unwrap();
    assert!(v.starts_with("Bearer"), "got {v}");
    assert!(v.contains("/.well-known/oauth-protected-resource"));
}

#[tokio::test]
async fn non_unauthorized_response_omits_www_authenticate() {
    let e = OAuthHttpError::invalid_request("bad");
    let resp = e.into_response();
    let (_, headers, _) = body_string(resp).await;
    assert!(headers.get(header::WWW_AUTHENTICATE).is_none());
}

#[tokio::test]
async fn with_redirect_emits_302_with_error_query() {
    let e = OAuthHttpError::access_denied("user said no")
        .with_redirect("https://app.example/cb", Some("xyz".into()));
    let resp = e.into_response();
    let status = resp.status();
    let location = resp
        .headers()
        .get(header::LOCATION)
        .expect("Location header")
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::TEMPORARY_REDIRECT,
        "{status}"
    );
    assert!(
        location.starts_with("https://app.example/cb?"),
        "{location}"
    );
    assert!(location.contains("error=access_denied"));
    assert!(location.contains("error_description="));
    assert!(location.contains("state=xyz"));
}

#[tokio::test]
async fn with_redirect_without_state_skips_state_param() {
    let e = OAuthHttpError::invalid_request("missing client_id")
        .with_redirect("https://app.example/cb", None);
    let resp = e.into_response();
    let location = resp
        .headers()
        .get(header::LOCATION)
        .expect("Location header")
        .to_str()
        .unwrap()
        .to_owned();
    assert!(location.contains("error=invalid_request"));
    assert!(!location.contains("state="));
}

#[tokio::test]
async fn server_error_response_body_logs_and_returns_500() {
    let e = OAuthHttpError::server_error("oops");
    let resp = e.into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
