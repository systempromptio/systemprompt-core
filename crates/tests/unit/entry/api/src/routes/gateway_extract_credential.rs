//! Unit tests for `routes::gateway::messages::extract_credential` — the
//! `x-api-key` / `Authorization` header parser that strips the `Bearer ` prefix
//! and trims whitespace.

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_api::routes::gateway::messages::extract_credential;

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
