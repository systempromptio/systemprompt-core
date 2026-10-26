//! Tests for `AuthorizationService` early-exit paths.
//!
//! The success path needs the global `Config` singleton plus a JWT
//! signed by the authority keyring. Coverage here targets the
//! deterministic short-circuits that fire when no token can be
//! extracted from the request: `authorize_service_access`,
//! `authorize_required_audience`, and `authorize_any_audience` all
//! return `UNAUTHORIZED` without touching `Config`.

use http::{HeaderMap, StatusCode};
use systemprompt_identifiers::ServiceName;
use systemprompt_oauth::services::jwt::AuthorizationService;

#[test]
fn authorize_service_access_returns_unauthorized_without_token() {
    let headers = HeaderMap::new();

    let err = AuthorizationService::authorize_service_access(
        systemprompt_test_fixtures::test_token_authority(
            systemprompt_test_fixtures::FIXTURE_JWT_ISSUER,
        )
        .as_ref(),
        &systemprompt_test_fixtures::test_config(),
        &headers,
        &ServiceName::new("mcp"),
    )
    .expect_err("absent token must reject");

    assert_eq!(err, StatusCode::UNAUTHORIZED);
}

#[test]
fn authorize_service_access_returns_unauthorized_for_basic_auth() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        "Basic Zm9vOmJhcg==".parse().expect("parse header"),
    );

    let err = AuthorizationService::authorize_service_access(
        systemprompt_test_fixtures::test_token_authority(
            systemprompt_test_fixtures::FIXTURE_JWT_ISSUER,
        )
        .as_ref(),
        &systemprompt_test_fixtures::test_config(),
        &headers,
        &ServiceName::new("api"),
    )
    .expect_err("basic auth must reject");

    assert_eq!(err, StatusCode::UNAUTHORIZED);
}

#[test]
fn authorize_required_audience_returns_unauthorized_without_token() {
    let headers = HeaderMap::new();

    let err = AuthorizationService::authorize_required_audience(
        systemprompt_test_fixtures::test_token_authority(
            systemprompt_test_fixtures::FIXTURE_JWT_ISSUER,
        )
        .as_ref(),
        &systemprompt_test_fixtures::test_config(),
        &headers,
        "api",
    )
    .expect_err("absent token must reject");

    assert_eq!(err, StatusCode::UNAUTHORIZED);
}

#[test]
fn authorize_any_audience_returns_unauthorized_without_token() {
    let headers = HeaderMap::new();

    let err = AuthorizationService::authorize_any_audience(
        systemprompt_test_fixtures::test_token_authority(
            systemprompt_test_fixtures::FIXTURE_JWT_ISSUER,
        )
        .as_ref(),
        &systemprompt_test_fixtures::test_config(),
        &headers,
        &["api", "web"],
    )
    .expect_err("absent token must reject");

    assert_eq!(err, StatusCode::UNAUTHORIZED);
}

#[test]
fn authorize_any_audience_returns_unauthorized_with_empty_allow_list() {
    let headers = HeaderMap::new();

    let err = AuthorizationService::authorize_any_audience(
        systemprompt_test_fixtures::test_token_authority(
            systemprompt_test_fixtures::FIXTURE_JWT_ISSUER,
        )
        .as_ref(),
        &systemprompt_test_fixtures::test_config(),
        &headers,
        &[],
    )
    .expect_err("absent token must reject before audience check");

    assert_eq!(err, StatusCode::UNAUTHORIZED);
}
