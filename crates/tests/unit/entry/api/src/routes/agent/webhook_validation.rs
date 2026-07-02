//! Unit tests for webhook payload sanitisation and serialisability checks.

use serde_json::json;
use systemprompt_api::routes::agent::contexts::webhook::test_api::{
    sanitize_payload, validate_json_serializable,
};

#[test]
fn small_payload_validates() {
    let payload = json!({"task": {"id": "t1"}, "artifacts": null});
    assert!(validate_json_serializable(&payload).is_ok());
}

#[test]
fn oversized_payload_is_rejected() {
    let big = "x".repeat(99_000);
    let items: Vec<serde_json::Value> = (0..12).map(|_| json!(big.clone())).collect();
    let payload = json!({ "items": items });
    let err = validate_json_serializable(&payload).expect_err("payload should exceed limit");
    assert!(err.contains("Payload too large"), "unexpected error: {err}");
}

#[test]
fn long_strings_are_truncated_with_marker() {
    let value = json!("a".repeat(50));
    let sanitized = sanitize_payload(&value, 10);
    let text = sanitized.as_str().expect("string");
    assert!(text.starts_with("aaaaaaaaaa..."));
    assert!(text.contains("[truncated from 50 bytes]"));
}

#[test]
fn short_strings_pass_through_unchanged() {
    let value = json!("short");
    assert_eq!(sanitize_payload(&value, 10), json!("short"));
}

#[test]
fn nested_arrays_and_objects_are_sanitized_recursively() {
    let value = json!({
        "outer": [{"inner": "b".repeat(20)}],
        "count": 3,
        "flag": true,
        "nothing": null,
    });
    let sanitized = sanitize_payload(&value, 5);
    let inner = sanitized["outer"][0]["inner"].as_str().expect("string");
    assert!(inner.contains("[truncated from 20 bytes]"));
    assert_eq!(sanitized["count"], json!(3));
    assert_eq!(sanitized["flag"], json!(true));
    assert_eq!(sanitized["nothing"], json!(null));
}
