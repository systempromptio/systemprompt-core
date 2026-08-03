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

// The `Unsupported` rejection arms: a body whose shape the wire does not
// define must be refused outright rather than silently reinterpreted, or the
// gateway forwards something the caller never wrote.
fn unsupported_field(body: &str) -> &'static str {
    match AnthropicMessagesInbound.parse_request(&Bytes::from(body.to_owned())) {
        Err(InboundParseError::Unsupported { field, .. }) => field,
        other => panic!("expected an Unsupported rejection, got {other:?}"),
    }
}

#[test]
fn a_system_prompt_that_is_neither_string_nor_array_is_refused() {
    let body = r#"{"model":"m","max_tokens":8,"system":42,"messages":[]}"#;

    assert_eq!(unsupported_field(body), "system");
}

#[test]
fn a_system_prompt_may_be_a_string_or_an_array_of_blocks() {
    let as_string = r#"{"model":"m","max_tokens":8,"system":"be brief","messages":[]}"#;
    let parsed = AnthropicMessagesInbound
        .parse_request(&Bytes::from(as_string))
        .expect("a string system prompt is the common form");
    assert_eq!(parsed.system.as_deref(), Some("be brief"));

    let as_blocks = r#"{"model":"m","max_tokens":8,
        "system":[{"type":"text","text":"be"},{"type":"text","text":"brief"}],
        "messages":[]}"#;
    let parsed = AnthropicMessagesInbound
        .parse_request(&Bytes::from(as_blocks))
        .expect("an array of text blocks is the cacheable form");
    assert!(
        parsed.system.as_deref().is_some_and(|s| s.contains("be")),
        "{:?}",
        parsed.system
    );
}

#[test]
fn an_empty_system_block_array_yields_no_system_prompt() {
    let body = r#"{"model":"m","max_tokens":8,"system":[],"messages":[]}"#;

    let parsed = AnthropicMessagesInbound
        .parse_request(&Bytes::from(body))
        .expect("an empty array is well-formed");

    assert!(
        parsed.system.is_none(),
        "an empty array must not become an empty system prompt"
    );
}

#[test]
fn a_message_content_of_an_undefined_shape_is_refused() {
    let body = r#"{"model":"m","max_tokens":8,
        "messages":[{"role":"user","content":17}]}"#;

    assert_eq!(unsupported_field(body), "messages[].content");
}

#[test]
fn an_image_source_type_the_wire_does_not_define_is_refused() {
    let body = r#"{"model":"m","max_tokens":8,"messages":[{"role":"user","content":[
        {"type":"image","source":{"type":"ftp","data":"AAAA"}}]}]}"#;

    assert_eq!(unsupported_field(body), "image.source.type");
}

#[test]
fn both_defined_image_source_types_parse() {
    let base64 = r#"{"model":"m","max_tokens":8,"messages":[{"role":"user","content":[
        {"type":"image","source":{"type":"base64","media_type":"image/png","data":"AAAA"}}]}]}"#;
    let parsed = AnthropicMessagesInbound
        .parse_request(&Bytes::from(base64))
        .expect("base64 is the documented inline form");
    assert_eq!(parsed.messages.len(), 1);

    let url = r#"{"model":"m","max_tokens":8,"messages":[{"role":"user","content":[
        {"type":"image","source":{"type":"url","url":"https://example.test/i.png"}}]}]}"#;
    let parsed = AnthropicMessagesInbound
        .parse_request(&Bytes::from(url))
        .expect("url is the documented remote form");
    assert_eq!(parsed.messages.len(), 1);
}

#[test]
fn a_plain_string_message_content_is_accepted() {
    let body = r#"{"model":"m","max_tokens":8,"messages":[{"role":"user","content":"hi"}]}"#;

    let parsed = AnthropicMessagesInbound
        .parse_request(&Bytes::from(body))
        .expect("the shorthand string form is valid");

    assert!(matches!(
        parsed.messages[0].content.first(),
        Some(CanonicalContent::Text(t)) if t == "hi"
    ));
}
