//! Unit tests for `services::gateway::audit::payload` — payload sizing,
//! JSON-vs-text fallback, oversize truncation, and the UTF-8-safe tool-input
//! cap.

use bytes::Bytes;
use systemprompt_api::services::gateway::audit::payload::{slice_payload, truncate_for_tool_input};

#[test]
fn small_valid_json_returns_value_no_excerpt() {
    let body = br#"{"hello":"world","n":3}"#;
    let (json, excerpt, truncated, bytes) = slice_payload(&Bytes::from_static(body));
    assert!(json.is_some(), "json was None");
    assert_eq!(json.unwrap()["hello"], "world");
    assert!(excerpt.is_none());
    assert!(!truncated);
    assert_eq!(bytes as usize, body.len());
}

#[test]
fn small_invalid_json_falls_back_to_text_excerpt() {
    let body = b"not json at all";
    let (json, excerpt, truncated, bytes) = slice_payload(&Bytes::from_static(body));
    assert!(json.is_none());
    assert_eq!(excerpt.as_deref(), Some("not json at all"));
    assert!(!truncated);
    assert_eq!(bytes as usize, body.len());
}

#[test]
fn empty_body_yields_text_excerpt() {
    let (json, excerpt, truncated, bytes) = slice_payload(&Bytes::new());
    assert!(json.is_none());
    assert_eq!(excerpt.as_deref(), Some(""));
    assert!(!truncated);
    assert_eq!(bytes, 0);
}

#[test]
fn oversize_payload_is_truncated_with_marker() {
    let payload_cap = 256 * 1024;
    let len = payload_cap + 5_000;
    let body = Bytes::from(vec![b'a'; len]);
    let (json, excerpt, truncated, bytes) = slice_payload(&body);
    assert!(json.is_none());
    assert!(truncated, "expected truncated");
    let e = excerpt.expect("excerpt present");
    assert!(e.contains("<truncated"));
    assert_eq!(bytes as usize, len);
}

#[test]
fn truncate_for_tool_input_small_input_unchanged() {
    let s = "small input";
    assert_eq!(truncate_for_tool_input(s), s);
}

#[test]
fn truncate_for_tool_input_at_cap_unchanged() {
    let cap = 64 * 1024;
    let s = "x".repeat(cap);
    assert_eq!(truncate_for_tool_input(&s).len(), cap);
}

#[test]
fn truncate_for_tool_input_over_cap_emits_marker() {
    let cap = 64 * 1024;
    let s = "y".repeat(cap + 1_000);
    let out = truncate_for_tool_input(&s);
    assert!(out.len() < s.len());
    assert!(out.contains("<truncated"));
    assert!(out.contains("bytes>"));
}

#[test]
fn truncate_for_tool_input_handles_utf8_boundary() {
    let cap = 64 * 1024;
    // Place a 4-byte codepoint straddling the cap so a naive `&s[..cap]` would
    // panic — the function must walk back to a char boundary.
    let mut s = "a".repeat(cap - 2);
    s.push('🦀'); // 4-byte UTF-8
    s.push_str(&"b".repeat(2_000));
    let out = truncate_for_tool_input(&s);
    assert!(out.contains("<truncated"));
    assert!(out.is_char_boundary(0));
}
