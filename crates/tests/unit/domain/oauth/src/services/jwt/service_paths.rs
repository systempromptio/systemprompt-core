// Config-backed success and audience-rejection paths for
// AuthenticationService / AuthorizationService / AuthService, driven by
// authority-signed fixture JWTs (issuer "test", audience "api").

use http::{HeaderMap, StatusCode};
use systemprompt_identifiers::UserId;
use systemprompt_oauth::services::jwt::{
    AuthService, AuthenticationService, AuthorizationService, extract_bearer_token,
    extract_cookie_token,
};
use systemprompt_test_fixtures::{ensure_test_bootstrap, mint_admin_jwt};
use uuid::Uuid;

fn bearer_headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        format!("Bearer {token}").parse().expect("header"),
    );
    headers
}

fn minted_headers(email: &str) -> (UserId, HeaderMap) {
    let user_id = UserId::new(Uuid::new_v4().to_string());
    let token = mint_admin_jwt(&user_id, email, "https://issuer.test");
    (user_id, bearer_headers(token.as_str()))
}

#[test]
fn authenticate_accepts_authority_signed_token() {
    ensure_test_bootstrap();
    let (user_id, headers) = minted_headers("authn@test.invalid");

    let user = AuthenticationService::authenticate(&headers).expect("authenticate");
    assert_eq!(user.id.to_string(), user_id.as_str());
    assert_eq!(user.email, "authn@test.invalid");
}

#[test]
fn authenticate_rejects_garbage_token() {
    ensure_test_bootstrap();
    let headers = bearer_headers("garbage.token.value");

    let err = AuthenticationService::authenticate(&headers).expect_err("garbage");
    assert_eq!(err, StatusCode::UNAUTHORIZED);
}

#[test]
fn authenticate_rejects_non_uuid_subject() {
    ensure_test_bootstrap();
    let user_id = UserId::new("service-account");
    let token = mint_admin_jwt(&user_id, "svc@test.invalid", "https://issuer.test");

    let err = AuthenticationService::authenticate(&bearer_headers(token.as_str()))
        .expect_err("non-uuid sub");
    assert_eq!(err, StatusCode::UNAUTHORIZED);
}

#[test]
fn authorize_service_access_accepts_valid_token() {
    ensure_test_bootstrap();
    let (user_id, headers) = minted_headers("svcacc@test.invalid");

    let user = AuthorizationService::authorize_service_access(&headers, "mcp").expect("authorized");
    assert_eq!(user.id.to_string(), user_id.as_str());
}

#[test]
fn authorize_service_access_rejects_invalid_jwt() {
    ensure_test_bootstrap();
    let headers = bearer_headers("still.not.ajwt");

    let err =
        AuthorizationService::authorize_service_access(&headers, "mcp").expect_err("invalid jwt");
    assert_eq!(err, StatusCode::UNAUTHORIZED);
}

#[test]
fn authorize_required_audience_accepts_api() {
    ensure_test_bootstrap();
    let (user_id, headers) = minted_headers("reqaud@test.invalid");

    let user =
        AuthorizationService::authorize_required_audience(&headers, "api").expect("authorized");
    assert_eq!(user.id.to_string(), user_id.as_str());
}

#[test]
fn authorize_required_audience_rejects_missing_audience() {
    ensure_test_bootstrap();
    let (_user_id, headers) = minted_headers("noaud@test.invalid");

    let err = AuthorizationService::authorize_required_audience(&headers, "hook")
        .expect_err("audience absent from claims");
    assert_eq!(err, StatusCode::FORBIDDEN);
}

#[test]
fn authorize_required_audience_treats_unknown_string_as_resource_and_rejects() {
    ensure_test_bootstrap();
    let (_user_id, headers) = minted_headers("badaud@test.invalid");

    let err = AuthorizationService::authorize_required_audience(&headers, "")
        .expect_err("empty resource audience is absent from claims");
    assert_eq!(err, StatusCode::FORBIDDEN);
}

#[test]
fn authorize_any_audience_accepts_when_one_matches() {
    ensure_test_bootstrap();
    let (user_id, headers) = minted_headers("anyaud@test.invalid");

    let user = AuthorizationService::authorize_any_audience(&headers, &["hook", "api"])
        .expect("authorized");
    assert_eq!(user.id.to_string(), user_id.as_str());
}

#[test]
fn authorize_any_audience_rejects_when_none_match() {
    ensure_test_bootstrap();
    let (_user_id, headers) = minted_headers("noneaud@test.invalid");

    let err =
        AuthorizationService::authorize_any_audience(&headers, &["hook"]).expect_err("no overlap");
    assert_eq!(err, StatusCode::FORBIDDEN);
}

#[test]
fn authorize_any_audience_skips_invalid_entries_and_still_matches() {
    ensure_test_bootstrap();
    let (user_id, headers) = minted_headers("mixaud@test.invalid");

    let user = AuthorizationService::authorize_any_audience(&headers, &["", "api"])
        .expect("invalid entries are skipped, api still matches");
    assert_eq!(user.id.to_string(), user_id.as_str());
}

#[test]
fn auth_service_facade_delegates_all_paths() {
    ensure_test_bootstrap();
    let (user_id, headers) = minted_headers("facade@test.invalid");

    let token = AuthService::extract_bearer_token(&headers).expect("bearer");
    assert_eq!(token.split('.').count(), 3);
    let err = AuthService::extract_bearer_token(&HeaderMap::new()).expect_err("no header");
    assert_eq!(err, StatusCode::UNAUTHORIZED);

    let user = AuthService::authenticate(&headers).expect("authenticate");
    assert_eq!(user.id.to_string(), user_id.as_str());
    let user = AuthService::authorize_service_access(&headers, "api").expect("service access");
    assert_eq!(user.id.to_string(), user_id.as_str());
    let user = AuthService::authorize_required_audience(&headers, "api").expect("required aud");
    assert_eq!(user.id.to_string(), user_id.as_str());
}

#[test]
fn extract_cookie_token_finds_access_token_among_cookies() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        "theme=dark; access_token=tok123; lang=en"
            .parse()
            .expect("header"),
    );
    assert_eq!(extract_cookie_token(&headers).expect("token"), "tok123");
}

#[test]
fn extract_cookie_token_errors_without_cookie_or_match() {
    let err = extract_cookie_token(&HeaderMap::new()).expect_err("no cookie header");
    assert!(err.to_string().contains("Cookie header missing"));

    let mut headers = HeaderMap::new();
    headers.insert("cookie", "theme=dark".parse().expect("header"));
    let err = extract_cookie_token(&headers).expect_err("no access_token cookie");
    assert!(err.to_string().contains("Access token not found"));
}

#[test]
fn extract_bearer_token_free_function_maps_error() {
    let err = extract_bearer_token(&HeaderMap::new()).expect_err("no header");
    assert!(err.to_string().contains("Authorization header"));
}
