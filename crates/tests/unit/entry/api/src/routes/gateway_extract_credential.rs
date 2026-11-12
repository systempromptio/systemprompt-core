//! Unit tests for `routes::gateway::messages::extract_credential` — the
//! `x-api-key` / `Authorization` header parser that strips the `Bearer ` prefix
//! and trims whitespace.

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_api::routes::gateway::messages::{
    caller_auth, extract_credential, presented_credential,
};

fn hm(name: &'static str, value: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(name, HeaderValue::from_str(value).unwrap());
    h
}

#[test]
fn extracts_bare_api_key() {
    let h = hm("x-api-key", "sk-abc123");
    assert_eq!(extract_credential(&h).as_deref(), Some("sk-abc123"));
}

#[test]
fn extracts_bearer_token_from_authorization() {
    let h = hm("authorization", "Bearer eyJabc");
    assert_eq!(extract_credential(&h).as_deref(), Some("eyJabc"));
}

#[test]
fn trims_surrounding_whitespace() {
    let h = hm("x-api-key", "  sk-abc  ");
    assert_eq!(extract_credential(&h).as_deref(), Some("sk-abc"));
}

#[test]
fn authorization_header_takes_precedence_over_api_key() {
    let mut h = HeaderMap::new();
    h.insert("x-api-key", HeaderValue::from_static("first"));
    h.insert("authorization", HeaderValue::from_static("Bearer second"));
    assert_eq!(extract_credential(&h).as_deref(), Some("second"));
}

#[test]
fn returns_none_when_no_headers_present() {
    let h = HeaderMap::new();
    assert!(extract_credential(&h).is_none());
}

#[test]
fn returns_none_for_empty_value() {
    let h = hm("x-api-key", "");
    assert!(extract_credential(&h).is_none());
}

#[test]
fn returns_none_for_whitespace_only_value() {
    let h = hm("x-api-key", "   ");
    assert!(extract_credential(&h).is_none());
}

#[test]
fn returns_none_for_bare_bearer_prefix() {
    let h = hm("authorization", "Bearer ");
    assert!(extract_credential(&h).is_none());
}

#[test]
fn returns_none_for_non_ascii_header_value() {
    let mut h = HeaderMap::new();
    h.insert(
        "x-api-key",
        HeaderValue::from_bytes(&[0xFF, 0xFE, 0xFD]).unwrap(),
    );
    assert!(extract_credential(&h).is_none());
}

#[test]
fn the_dedicated_header_is_the_identity_and_authorization_is_not_read() {
    let mut h = HeaderMap::new();
    h.insert(
        "authorization",
        HeaderValue::from_static("Bearer upstream-credential"),
    );
    h.insert(
        "x-systemprompt-authorization",
        HeaderValue::from_static("Bearer sp-live-abc"),
    );
    assert_eq!(extract_credential(&h).as_deref(), Some("sp-live-abc"));
}

#[test]
fn an_empty_dedicated_header_does_not_fall_back_to_authorization() {
    let mut h = HeaderMap::new();
    h.insert(
        "authorization",
        HeaderValue::from_static("Bearer upstream-credential"),
    );
    h.insert("x-systemprompt-authorization", HeaderValue::from_static(""));
    assert!(extract_credential(&h).is_none());
}

#[test]
fn the_callers_authorization_is_captured_only_beside_the_dedicated_header() {
    let alone = hm("authorization", "Bearer upstream-credential");
    assert!(caller_auth(&alone).is_none());

    let mut both = HeaderMap::new();
    both.insert(
        "authorization",
        HeaderValue::from_static("Bearer upstream-credential"),
    );
    both.insert(
        "x-systemprompt-authorization",
        HeaderValue::from_static("sp-live-abc"),
    );
    let auth = caller_auth(&both).expect("captured");
    assert!(auth.is_bearer());
    assert_eq!(auth.value, "upstream-credential");
    assert!(!format!("{auth:?}").contains("upstream-credential"));

    let mut no_bearer = HeaderMap::new();
    no_bearer.insert(
        "authorization",
        HeaderValue::from_static("upstream-credential"),
    );
    no_bearer.insert(
        "x-systemprompt-authorization",
        HeaderValue::from_static("sp-live-abc"),
    );
    assert!(caller_auth(&no_bearer).is_none());
}

#[test]
fn a_presented_credential_carries_the_clients_headers_and_none_of_ours() {
    let mut h = HeaderMap::new();
    for (name, value) in [
        ("authorization", "Bearer upstream-credential"),
        ("x-systemprompt-authorization", "sp-live-abc"),
        ("x-systemprompt-memory", "off"),
        ("x-session-id", "session-1"),
        ("x-context-id", "context-1"),
        ("traceparent", "00-aaaa-bbbb-01"),
        ("x-api-key", "sp-live-other"),
        ("cookie", "access_token=secret"),
        ("x-forwarded-for", "203.0.113.7"),
        ("host", "gateway.invalid"),
        ("content-length", "42"),
        ("accept-encoding", "gzip"),
        ("user-agent", "client/1.0"),
        ("anthropic-beta", "capability-a"),
        ("x-stainless-os", "Linux"),
        ("x-app", "cli"),
    ] {
        h.insert(name, HeaderValue::from_static(value));
    }

    let presented = presented_credential(&h).expect("presented");

    assert_eq!(presented.auth.value, "upstream-credential");
    let mut names: Vec<&str> = presented.headers.iter().map(|(n, _)| n.as_str()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec!["anthropic-beta", "user-agent", "x-app", "x-stainless-os"]
    );
}

#[test]
fn nothing_is_presented_without_the_dedicated_header() {
    let h = hm("authorization", "Bearer sp-live-abc");
    assert!(presented_credential(&h).is_none());
}
