//! Unit tests for `services::gateway::audit::payload` — payload sizing,
//! JSON-vs-text fallback, oversize truncation, the always-present SHA-256
//! digest, the profile-supplied cap, the prepared-tools extraction, and the
//! UTF-8-safe tool-input cap.

use bytes::Bytes;
use serde_json::json;
use sha2::{Digest, Sha256};
use systemprompt_api::services::gateway::audit::payload::{
    excerpt_payload, prepared_tools, slice_payload, truncate_for_tool_input,
};
use systemprompt_models::profile::AuditConfig;

const PAYLOAD_CAP: usize = AuditConfig::DEFAULT_PAYLOAD_CAP_BYTES;
const EXCERPT_BYTES: usize = 8 * 1024;

fn expected_digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[test]
fn small_valid_json_returns_value_no_excerpt() {
    let body = br#"{"hello":"world","n":3}"#;
    let capture = slice_payload(&Bytes::from_static(body), PAYLOAD_CAP);
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
    let capture = slice_payload(&Bytes::from_static(body), PAYLOAD_CAP);
    assert!(capture.json.is_none());
    assert_eq!(capture.excerpt.as_deref(), Some("not json at all"));
    assert!(!capture.truncated);
    assert_eq!(capture.byte_len as usize, body.len());
    assert_eq!(capture.sha256, expected_digest(body));
}

#[test]
fn empty_body_yields_text_excerpt() {
    let capture = slice_payload(&Bytes::new(), PAYLOAD_CAP);
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
    let capture = slice_payload(&Bytes::from(body.clone()), PAYLOAD_CAP);
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
    let capture = slice_payload(&body, PAYLOAD_CAP);

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
    let capture = slice_payload(&body, PAYLOAD_CAP);
    let excerpt = capture.excerpt.expect("excerpt present");
    let dropped = len - EXCERPT_BYTES - EXCERPT_BYTES;
    assert!(
        excerpt.contains(&format!("<truncated {dropped} bytes>")),
        "marker must report len - head - tail"
    );
}

#[test]
fn custom_cap_widens_what_is_stored_whole() {
    let len = PAYLOAD_CAP + 5_000;
    let body = Bytes::from(format!(r#"{{"k":"{}"}}"#, "a".repeat(len)));
    let capture = slice_payload(&body, 4 * PAYLOAD_CAP);
    assert!(
        !capture.truncated,
        "a 4 MiB cap keeps a 1 MiB + 5 KB body whole"
    );
    assert!(capture.json.is_some());
    assert_eq!(capture.byte_len as usize, body.len());
}

#[test]
fn custom_cap_below_default_truncates_sooner() {
    let cap = 128 * 1024;
    let body = Bytes::from(vec![b'a'; cap + 1]);
    let capture = slice_payload(&body, cap);
    assert!(capture.truncated, "one byte over the custom cap truncates");
    assert!(capture.excerpt.is_some_and(|e| e.contains("<truncated")));
}

#[test]
fn cap_never_drops_below_the_floor() {
    let body = Bytes::from(vec![b'a'; AuditConfig::MIN_PAYLOAD_CAP_BYTES]);
    let capture = slice_payload(&body, 1);
    assert!(
        !capture.truncated,
        "a cap under the floor is raised to it, never applied as given"
    );
}

#[test]
fn prepared_tools_returns_the_exact_tools_array() {
    let body = json!({
        "model": "m",
        "tools": [{"name": "read", "input_schema": {"type": "object"}}],
        "messages": []
    });
    let tools = prepared_tools(&serde_json::to_vec(&body).unwrap()).expect("tools present");
    assert_eq!(tools, body["tools"]);
}

#[test]
fn prepared_tools_is_none_without_an_array() {
    assert!(prepared_tools(br#"{"model":"m","messages":[]}"#).is_none());
    assert!(prepared_tools(br#"{"tools":{"not":"an array"}}"#).is_none());
    assert!(prepared_tools(b"not json").is_none());
}

#[test]
fn excerpt_payload_never_keeps_the_body() {
    let body = json!({"model": "m", "max_tokens": 1, "messages": [{"role": "user", "content": "hi"}]});
    let bytes = Bytes::from(serde_json::to_vec(&body).unwrap());
    let capture = excerpt_payload(&bytes);
    assert!(capture.json.is_none());
    assert!(capture.truncated);
    assert_eq!(capture.byte_len as usize, bytes.len());
    assert_eq!(capture.sha256, slice_payload(&bytes, 1 << 20).sha256);
    let excerpt = capture.excerpt.expect("excerpt");
    assert_eq!(excerpt.as_bytes(), &bytes[..], "a small body is excerpted whole");
}

#[test]
fn excerpt_payload_keeps_head_and_tail_of_a_large_body() {
    let bytes = Bytes::from(vec![b'x'; 40 * 1024]);
    let excerpt = excerpt_payload(&bytes).excerpt.expect("excerpt");
    assert!(excerpt.contains("...<truncated 24576 bytes>..."));
    assert!(excerpt.len() < bytes.len());
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
