//! Unit tests for `services::gateway::audit::payload` — payload sizing,
//! JSON-vs-text fallback, oversize truncation, the always-present SHA-256
//! digest, and the UTF-8-safe tool-input cap.

use bytes::Bytes;
use sha2::{Digest, Sha256};
use systemprompt_api::services::gateway::audit::payload::{slice_payload, truncate_for_tool_input};

const PAYLOAD_CAP: usize = 1024 * 1024;
const EXCERPT_BYTES: usize = 8 * 1024;

fn expected_digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[test]
fn small_valid_json_returns_value_no_excerpt() {
    let body = br#"{"hello":"world","n":3}"#;
    let capture = slice_payload(&Bytes::from_static(body));
    assert!(capture.json.is_some(), "json was None");
    assert_eq!(capture.json.unwrap()["hello"], "world");
    assert!(capture.excerpt.is_none());
    assert!(!capture.truncated);
    assert_eq!(capture.byte_len as usize, body.len());
    assert_eq!(capture.sha256, expected_digest(body));
}

#[test]
fn small_invalid_json_falls_back_to_text_excerpt() {
    let body = b"not json at all";
    let capture = slice_payload(&Bytes::from_static(body));
    assert!(capture.json.is_none());
    assert_eq!(capture.excerpt.as_deref(), Some("not json at all"));
    assert!(!capture.truncated);
    assert_eq!(capture.byte_len as usize, body.len());
    assert_eq!(capture.sha256, expected_digest(body));
}

#[test]
fn empty_body_yields_text_excerpt() {
    let capture = slice_payload(&Bytes::new());
    assert!(capture.json.is_none());
    assert_eq!(capture.excerpt.as_deref(), Some(""));
    assert!(!capture.truncated);
    assert_eq!(capture.byte_len, 0);
    assert_eq!(capture.sha256, expected_digest(b""));
}

#[test]
fn one_mib_json_body_is_still_stored_as_structured_json() {
    let overhead = r#"{"k":""}"#.len();
    let body = format!(r#"{{"k":"{}"}}"#, "a".repeat(PAYLOAD_CAP - overhead));
    assert_eq!(body.len(), PAYLOAD_CAP);
    let capture = slice_payload(&Bytes::from(body.clone()));
    assert!(!capture.truncated, "a body at the cap is not truncated");
    assert!(capture.json.is_some(), "1 MiB JSON must still be parsed");
    assert!(capture.excerpt.is_none());
    assert_eq!(capture.byte_len as usize, body.len());
    assert_eq!(capture.sha256, expected_digest(body.as_bytes()));
}

#[test]
fn oversize_payload_keeps_head_and_tail_and_still_digests_full_body() {
    let len = PAYLOAD_CAP + 5_000;
    let mut raw = vec![b'a'; len];
    raw[len - 4..].copy_from_slice(b"tail");
    let body = Bytes::from(raw.clone());
    let capture = slice_payload(&body);

    assert!(capture.json.is_none());
    assert!(capture.truncated, "expected truncated");
    assert_eq!(capture.byte_len as usize, len);
    assert_eq!(
        capture.sha256,
        expected_digest(&raw),
        "digest covers the full body, not the excerpt"
    );

    let excerpt = capture.excerpt.expect("excerpt present");
    assert!(excerpt.starts_with(&"a".repeat(64)), "head present");
    assert!(excerpt.ends_with("tail"), "tail present");
    assert!(excerpt.contains("<truncated"));
}

#[test]
fn truncation_marker_counts_bytes_dropped_not_bytes_after_head() {
    let len = PAYLOAD_CAP + 5_000;
    let body = Bytes::from(vec![b'a'; len]);
    let capture = slice_payload(&body);
    let excerpt = capture.excerpt.expect("excerpt present");
    let dropped = len - EXCERPT_BYTES - EXCERPT_BYTES;
    assert!(
        excerpt.contains(&format!("<truncated {dropped} bytes>")),
        "marker must report len - head - tail"
    );
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
