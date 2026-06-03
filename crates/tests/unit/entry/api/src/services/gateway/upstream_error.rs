//! `extract_upstream_message` recovers the provider's `error.message` from a
//! JSON error body (the shape OpenAI, Anthropic, and Gemini share) and bounds
//! the fallback so a non-JSON or oversized body cannot flood logs or responses.

use systemprompt_api::services::gateway::protocol::outbound::{
    UpstreamError, extract_upstream_message,
};

#[test]
fn extracts_provider_error_message_from_json() {
    let body = r#"{"error":{"message":"Unsupported parameter: 'max_tokens'","type":"invalid_request_error"}}"#;
    assert_eq!(
        extract_upstream_message(body),
        "Unsupported parameter: 'max_tokens'"
    );
}

#[test]
fn falls_back_to_raw_body_when_no_error_message() {
    assert_eq!(
        extract_upstream_message("plain text error"),
        "plain text error"
    );
    let no_msg = r#"{"detail":"nope"}"#;
    assert_eq!(extract_upstream_message(no_msg), no_msg);
}

#[test]
fn truncates_oversized_fallback_body_to_500_chars() {
    let big = "x".repeat(2000);
    assert_eq!(extract_upstream_message(&big).chars().count(), 500);
}

#[test]
fn status_error_display_carries_provider_status_and_message() {
    let shown = UpstreamError::Status {
        provider: "anthropic",
        status: 400,
        message: "bad request".to_owned(),
    }
    .to_string();
    assert!(shown.contains("anthropic"), "{shown}");
    assert!(shown.contains("400"), "{shown}");
    assert!(shown.contains("bad request"), "{shown}");
}
