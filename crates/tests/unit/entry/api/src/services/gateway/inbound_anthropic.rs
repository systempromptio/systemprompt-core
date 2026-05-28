//! Unit tests for the Anthropic Messages inbound adapter — wire name, JSON
//! parse failures, required-field validation, and error rendering escape.

use bytes::Bytes;
use http::StatusCode;
use systemprompt_api::services::gateway::protocol::canonical::{CanonicalContent, Role};
use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_api::services::gateway::protocol::inbound::{InboundAdapter, InboundParseError};

#[test]
fn wire_name_is_anthropic_messages() {
    let a = AnthropicMessagesInbound;
    assert_eq!(a.wire_name(), "anthropic.messages");
}

#[test]
fn default_streaming_content_type_is_sse() {
    let a = AnthropicMessagesInbound;
    assert_eq!(a.streaming_content_type(), "text/event-stream");
}

#[test]
fn parse_request_invalid_json_returns_invalidjson() {
    let a = AnthropicMessagesInbound;
    let err = a
        .parse_request(&Bytes::from_static(b"not json"))
        .expect_err("should fail");
    match err {
        InboundParseError::InvalidJson(_) => {},
        other => panic!("expected InvalidJson, got {other:?}"),
    }
}

#[test]
fn parse_request_missing_model() {
    let a = AnthropicMessagesInbound;
    let body = br#"{"max_tokens":100,"messages":[]}"#;
    let err = a
        .parse_request(&Bytes::from_static(body))
        .expect_err("should fail");
    match err {
        InboundParseError::MissingField("model") => {},
        other => panic!("expected MissingField(model), got {other:?}"),
    }
}

#[test]
fn parse_request_missing_max_tokens() {
    let a = AnthropicMessagesInbound;
    let body = br#"{"model":"claude","messages":[]}"#;
    let err = a
        .parse_request(&Bytes::from_static(body))
        .expect_err("should fail");
    match err {
        InboundParseError::MissingField("max_tokens") => {},
        other => panic!("expected MissingField(max_tokens), got {other:?}"),
    }
}

#[test]
fn parse_request_missing_messages() {
    let a = AnthropicMessagesInbound;
    let body = br#"{"model":"claude","max_tokens":100}"#;
    let err = a
        .parse_request(&Bytes::from_static(body))
        .expect_err("should fail");
    match err {
        InboundParseError::MissingField("messages") => {},
        other => panic!("expected MissingField(messages), got {other:?}"),
    }
}

#[test]
fn parse_request_minimal_valid_body() {
    let a = AnthropicMessagesInbound;
    let body = br#"{
        "model":"claude-3-5-sonnet",
        "max_tokens":1024,
        "messages":[{"role":"user","content":"hi"}]
    }"#;
    let req = a
        .parse_request(&Bytes::from_static(body))
        .expect("should parse");
    assert_eq!(req.model, "claude-3-5-sonnet");
    assert_eq!(req.max_tokens, 1024);
    assert_eq!(req.messages.len(), 1);
    assert_eq!(req.messages[0].role, Role::User);
    assert!(!req.stream);
    assert!(
        matches!(req.messages[0].content.first(), Some(CanonicalContent::Text(t)) if t == "hi")
    );
}

#[test]
fn parse_request_streaming_flag() {
    let a = AnthropicMessagesInbound;
    let body = br#"{
        "model":"m","max_tokens":1,"stream":true,
        "messages":[{"role":"user","content":"x"}]
    }"#;
    let req = a.parse_request(&Bytes::from_static(body)).expect("parse");
    assert!(req.stream);
}

#[test]
fn render_error_escapes_quotes_and_backslashes() {
    let a = AnthropicMessagesInbound;
    let out = a.render_error(StatusCode::BAD_REQUEST, r#"oops "quoted" \back"#);
    let s = String::from_utf8(out.to_vec()).unwrap();
    assert!(s.starts_with("{\"type\":\"error\""));
    assert!(s.contains("api_error"));
    assert!(s.contains(r#"\"quoted\""#), "got: {s}");
    assert!(s.contains(r#"\\back"#), "got: {s}");
}

#[test]
fn parse_request_with_system_string() {
    let a = AnthropicMessagesInbound;
    let body = br#"{
        "model":"m","max_tokens":1,"system":"you are helpful",
        "messages":[{"role":"user","content":"x"}]
    }"#;
    let req = a.parse_request(&Bytes::from_static(body)).expect("parse");
    assert_eq!(req.system.as_deref(), Some("you are helpful"));
}
