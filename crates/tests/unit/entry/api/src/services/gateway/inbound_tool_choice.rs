//! Malformed `tool_choice` must be rejected by every inbound surface.
//!
//! Each surface has its own grammar for the field. A value outside that
//! grammar used to be silently dropped, which let a client bug dispatch a
//! request the upstream API would have refused.

use bytes::Bytes;
use http::StatusCode;
use serde_json::{Value, json};
use systemprompt_api::services::gateway::protocol::canonical::CanonicalToolChoice;
use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_api::services::gateway::protocol::inbound::openai_chat::OpenAiChatInbound;
use systemprompt_api::services::gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;
use systemprompt_api::services::gateway::protocol::inbound::{InboundAdapter, InboundParseError};

// Why: each surface needs the same body with only `tool_choice` swapped, so
// the malformed forms are the only thing a case has to state.
fn anthropic_body(choice: &Value) -> Bytes {
    let body = json!({
        "model": "claude-sonnet-4",
        "max_tokens": 16,
        "messages": [{"role": "user", "content": "hi"}],
        "tools": [{"name": "f", "input_schema": {}}],
        "tool_choice": choice,
    });
    Bytes::from(body.to_string())
}

fn chat_body(choice: &Value) -> Bytes {
    let body = json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "hi"}],
        "tool_choice": choice,
    });
    Bytes::from(body.to_string())
}

fn responses_body(choice: &Value) -> Bytes {
    let body = json!({
        "model": "gpt-4o",
        "input": "hi",
        "tool_choice": choice,
    });
    Bytes::from(body.to_string())
}

fn assert_tool_choice_rejected(result: Result<impl Sized, InboundParseError>, expected: &str) {
    match result {
        Err(InboundParseError::Unsupported { field, detail }) => {
            assert_eq!(field, "tool_choice");
            assert_eq!(detail, expected);
        },
        Err(other) => panic!("wrong error: {other}"),
        Ok(_) => panic!("malformed tool_choice was accepted"),
    }
}

const ANTHROPIC_DETAIL: &str = "expected an object with type auto|any|tool";
const OPENAI_DETAIL: &str =
    "expected \"none\", \"auto\", \"required\", or an object with type function";

fn malformed_forms() -> Vec<Value> {
    vec![json!("required"), json!(3), json!(["auto"]), json!(true)]
}

#[test]
fn anthropic_rejects_non_object_tool_choice() {
    for form in malformed_forms() {
        assert_tool_choice_rejected(
            AnthropicMessagesInbound.parse_request(&anthropic_body(&form)),
            ANTHROPIC_DETAIL,
        );
    }
}

#[test]
fn anthropic_rejects_unknown_tool_choice_type() {
    assert_tool_choice_rejected(
        AnthropicMessagesInbound.parse_request(&anthropic_body(&json!({"type": "function"}))),
        ANTHROPIC_DETAIL,
    );
}

#[test]
fn anthropic_rejects_tool_type_without_name() {
    let err = AnthropicMessagesInbound
        .parse_request(&anthropic_body(&json!({"type": "tool"})))
        .expect_err("named tool without a name must be rejected");
    assert!(
        matches!(err, InboundParseError::Unsupported { field, .. } if field == "tool_choice"),
        "got: {err}"
    );
}

#[test]
fn anthropic_accepts_object_forms() {
    let req = AnthropicMessagesInbound
        .parse_request(&anthropic_body(&json!({"type": "any"})))
        .expect("parse");
    assert!(matches!(req.tool_choice, Some(CanonicalToolChoice::Any)));

    let req = AnthropicMessagesInbound
        .parse_request(&anthropic_body(&json!({"type": "tool", "name": "f"})))
        .expect("parse");
    assert!(matches!(req.tool_choice, Some(CanonicalToolChoice::Tool(ref n)) if n == "f"));
}

#[test]
fn openai_chat_rejects_values_outside_its_grammar() {
    for form in [
        json!("any"),
        json!(3),
        json!(["auto"]),
        json!({"type": "tool"}),
    ] {
        assert_tool_choice_rejected(
            OpenAiChatInbound.parse_request(&chat_body(&form)),
            OPENAI_DETAIL,
        );
    }
}

#[test]
fn openai_chat_accepts_string_and_function_forms() {
    let req = OpenAiChatInbound
        .parse_request(&chat_body(&json!("required")))
        .expect("parse");
    assert!(matches!(
        req.tool_choice,
        Some(CanonicalToolChoice::Required)
    ));

    let choice = json!({"type": "function", "function": {"name": "f"}});
    let req = OpenAiChatInbound
        .parse_request(&chat_body(&choice))
        .expect("parse");
    assert!(matches!(req.tool_choice, Some(CanonicalToolChoice::Tool(ref n)) if n == "f"));
}

#[test]
fn openai_responses_rejects_values_outside_its_grammar() {
    for form in [
        json!("any"),
        json!(3),
        json!(["auto"]),
        json!({"type": "tool"}),
    ] {
        assert_tool_choice_rejected(
            OpenAiResponsesInbound.parse_request(&responses_body(&form)),
            OPENAI_DETAIL,
        );
    }
}

#[test]
fn openai_responses_accepts_string_and_function_forms() {
    let req = OpenAiResponsesInbound
        .parse_request(&responses_body(&json!("auto")))
        .expect("parse");
    assert!(matches!(req.tool_choice, Some(CanonicalToolChoice::Auto)));

    let choice = json!({"type": "function", "name": "f"});
    let req = OpenAiResponsesInbound
        .parse_request(&responses_body(&choice))
        .expect("parse");
    assert!(matches!(req.tool_choice, Some(CanonicalToolChoice::Tool(ref n)) if n == "f"));
}

#[test]
fn openai_responses_accepts_hosted_tool_forms() {
    for form in [
        json!({"type": "file_search"}),
        json!({"type": "web_search_preview"}),
        json!({"type": "mcp", "server_label": "s"}),
        json!({"type": "allowed_tools", "mode": "auto", "tools": []}),
    ] {
        let req = OpenAiResponsesInbound
            .parse_request(&responses_body(&form))
            .expect("a hosted tool choice is valid client input");
        assert!(
            req.tool_choice.is_none(),
            "a hosted tool the gateway does not proxy carries no canonical constraint"
        );
    }
}

#[test]
fn rejections_render_as_invalid_request_errors() {
    let message = "unsupported value for tool_choice: expected an object";
    let body = AnthropicMessagesInbound.render_error(StatusCode::BAD_REQUEST, message);
    let v: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(v["error"]["type"], "invalid_request_error");

    let body = OpenAiChatInbound.render_error(StatusCode::BAD_REQUEST, message);
    let v: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(v["error"]["type"], "invalid_request_error");

    let body = OpenAiResponsesInbound.render_error(StatusCode::BAD_REQUEST, message);
    let v: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(v["error"]["type"], "invalid_request_error");
}
