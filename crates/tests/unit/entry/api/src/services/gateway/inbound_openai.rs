//! Unit tests for the OpenAI Responses inbound adapter — `wire_name`, error
//! rendering, defaults, and required-field validation.

use bytes::Bytes;
use http::StatusCode;
use systemprompt_api::services::gateway::protocol::inbound::InboundAdapter;
use systemprompt_api::services::gateway::protocol::inbound::InboundParseError;
use systemprompt_api::services::gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;

#[test]
fn wire_name_is_openai_responses() {
    let a = OpenAiResponsesInbound;
    assert_eq!(a.wire_name(), "openai.responses");
}

#[test]
fn parse_request_invalid_json() {
    let a = OpenAiResponsesInbound;
    let err = a
        .parse_request(&Bytes::from_static(b"not json"))
        .expect_err("should fail");
    assert!(matches!(err, InboundParseError::InvalidJson(_)));
}

#[test]
fn parse_request_missing_model_fails() {
    let a = OpenAiResponsesInbound;
    let body = br#"{"input":"hi"}"#;
    let err = a
        .parse_request(&Bytes::from_static(body))
        .expect_err("should fail");
    match err {
        InboundParseError::MissingField("model") => {},
        other => panic!("expected MissingField(model), got {other:?}"),
    }
}

#[test]
fn parse_request_minimal_model_only_uses_defaults() {
    let a = OpenAiResponsesInbound;
    let body = br#"{"model":"gpt-4o"}"#;
    let req = a.parse_request(&Bytes::from_static(body)).expect("parse");
    assert_eq!(req.model, "gpt-4o");
    assert_eq!(req.max_tokens, 4096);
    assert!(req.system.is_none());
    assert!(req.messages.is_empty());
    assert!(!req.stream);
}

#[test]
fn parse_request_with_instructions_populates_system() {
    let a = OpenAiResponsesInbound;
    let body = br#"{"model":"gpt-4o","instructions":"be brief"}"#;
    let req = a.parse_request(&Bytes::from_static(body)).expect("parse");
    assert_eq!(req.system.as_deref(), Some("be brief"));
}

#[test]
fn parse_request_empty_instructions_is_none() {
    let a = OpenAiResponsesInbound;
    let body = br#"{"model":"gpt-4o","instructions":""}"#;
    let req = a.parse_request(&Bytes::from_static(body)).expect("parse");
    assert!(req.system.is_none());
}

#[test]
fn parse_request_stop_as_string() {
    let a = OpenAiResponsesInbound;
    let body = br#"{"model":"gpt-4o","stop":"END"}"#;
    let req = a.parse_request(&Bytes::from_static(body)).expect("parse");
    assert_eq!(req.stop_sequences, vec!["END".to_owned()]);
}

#[test]
fn parse_request_stop_as_array() {
    let a = OpenAiResponsesInbound;
    let body = br#"{"model":"gpt-4o","stop":["A","B"]}"#;
    let req = a.parse_request(&Bytes::from_static(body)).expect("parse");
    assert_eq!(req.stop_sequences, vec!["A".to_owned(), "B".to_owned()]);
}

#[test]
fn parse_request_stream_flag() {
    let a = OpenAiResponsesInbound;
    let body = br#"{"model":"gpt-4o","stream":true}"#;
    let req = a.parse_request(&Bytes::from_static(body)).expect("parse");
    assert!(req.stream);
}

#[test]
fn render_error_escapes_quotes_and_backslashes() {
    let a = OpenAiResponsesInbound;
    let out = a.render_error(StatusCode::BAD_REQUEST, r#"oops "quoted" \back"#);
    let s = String::from_utf8(out.to_vec()).unwrap();
    assert!(s.starts_with("{\"error\""));
    assert!(s.contains(r#"\"quoted\""#));
    assert!(s.contains(r#"\\back"#));
}

#[test]
fn streaming_content_type_default() {
    let a = OpenAiResponsesInbound;
    assert_eq!(a.streaming_content_type(), "text/event-stream");
}
