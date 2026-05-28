//! Deeper parse-branch coverage for the OpenAI Responses inbound adapter:
//! input as string vs structured array, function_call/output, reasoning
//! items, tool choice variants, and content part types.

use bytes::Bytes;
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalToolChoice, ImageSource, Role,
};
use systemprompt_api::services::gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;
use systemprompt_api::services::gateway::protocol::inbound::{InboundAdapter, InboundParseError};

fn parse_ok(
    body: &[u8],
) -> systemprompt_api::services::gateway::protocol::canonical::CanonicalRequest {
    let a = OpenAiResponsesInbound;
    a.parse_request(&Bytes::copy_from_slice(body))
        .expect("parse")
}

#[test]
fn parse_input_as_string_becomes_single_user_message() {
    let req = parse_ok(br#"{"model":"gpt-4o","input":"hi there"}"#);
    assert_eq!(req.messages.len(), 1);
    assert_eq!(req.messages[0].role, Role::User);
    assert!(
        matches!(req.messages[0].content.first(), Some(CanonicalContent::Text(t)) if t == "hi there")
    );
}

#[test]
fn parse_input_unsupported_shape_errors() {
    let a = OpenAiResponsesInbound;
    let body = br#"{"model":"gpt-4o","input":42}"#;
    let err = a.parse_request(&Bytes::from_static(body)).expect_err("err");
    assert!(matches!(err, InboundParseError::Unsupported { field, .. } if field == "input"));
}

#[test]
fn parse_input_message_with_input_text_part() {
    let body = br#"{
        "model":"gpt-4o",
        "input":[{"type":"message","role":"user","content":[{"type":"input_text","text":"hi"}]}]
    }"#;
    let req = parse_ok(body);
    assert_eq!(req.messages.len(), 1);
    match req.messages[0].content.first() {
        Some(CanonicalContent::Text(t)) => assert_eq!(t, "hi"),
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn parse_input_message_with_input_image_url() {
    let body = br#"{
        "model":"gpt-4o",
        "input":[{"type":"message","role":"user","content":[{"type":"input_image","image_url":"https://x/y.png"}]}]
    }"#;
    let req = parse_ok(body);
    match req.messages[0].content.first() {
        Some(CanonicalContent::Image(ImageSource::Url(u))) => assert_eq!(u, "https://x/y.png"),
        other => panic!("expected image url, got {other:?}"),
    }
}

#[test]
fn parse_input_message_string_content() {
    let body = br#"{
        "model":"gpt-4o",
        "input":[{"type":"message","role":"assistant","content":"hello"}]
    }"#;
    let req = parse_ok(body);
    assert_eq!(req.messages[0].role, Role::Assistant);
    assert!(
        matches!(req.messages[0].content.first(), Some(CanonicalContent::Text(t)) if t == "hello")
    );
}

#[test]
fn parse_input_developer_role_maps_to_system() {
    let body = br#"{
        "model":"gpt-4o",
        "input":[{"type":"message","role":"developer","content":"be precise"}]
    }"#;
    let req = parse_ok(body);
    assert_eq!(req.messages[0].role, Role::System);
}

#[test]
fn parse_input_unknown_role_errors() {
    let a = OpenAiResponsesInbound;
    let body = br#"{
        "model":"gpt-4o",
        "input":[{"type":"message","role":"narrator","content":"x"}]
    }"#;
    let err = a.parse_request(&Bytes::from_static(body)).expect_err("err");
    assert!(matches!(err, InboundParseError::Unsupported { field, .. } if field == "input[].role"));
}

#[test]
fn parse_input_unknown_item_type_errors() {
    let a = OpenAiResponsesInbound;
    let body = br#"{
        "model":"gpt-4o",
        "input":[{"type":"audio","data":"x"}]
    }"#;
    let err = a.parse_request(&Bytes::from_static(body)).expect_err("err");
    assert!(matches!(err, InboundParseError::Unsupported { field, .. } if field == "input[].type"));
}

#[test]
fn parse_function_call_item_becomes_tool_use() {
    let body = br#"{
        "model":"gpt-4o",
        "input":[{"type":"function_call","call_id":"call_1","name":"search","arguments":"{\"q\":\"r\"}"}]
    }"#;
    let req = parse_ok(body);
    assert_eq!(req.messages.len(), 1);
    assert_eq!(req.messages[0].role, Role::Assistant);
    match req.messages[0].content.first() {
        Some(CanonicalContent::ToolUse { id, name, input }) => {
            assert_eq!(id, "call_1");
            assert_eq!(name, "search");
            assert_eq!(input["q"], "r");
        },
        other => panic!("expected tool_use, got {other:?}"),
    }
}

#[test]
fn parse_function_call_output_becomes_tool_result() {
    let body = br#"{
        "model":"gpt-4o",
        "input":[{"type":"function_call_output","call_id":"call_1","output":"42"}]
    }"#;
    let req = parse_ok(body);
    assert_eq!(req.messages.len(), 1);
    assert_eq!(req.messages[0].role, Role::Tool);
    match req.messages[0].content.first() {
        Some(CanonicalContent::ToolResult {
            tool_use_id,
            content,
            is_error,
        }) => {
            assert_eq!(tool_use_id, "call_1");
            assert!(!is_error);
            assert!(matches!(content.first(), Some(CanonicalContent::Text(t)) if t == "42"));
        },
        other => panic!("expected tool_result, got {other:?}"),
    }
}

#[test]
fn parse_reasoning_item_with_summary_becomes_thinking() {
    let body = br#"{
        "model":"gpt-4o",
        "input":[{"type":"reasoning","summary":[{"text":"a"},{"text":"b"}]}]
    }"#;
    let req = parse_ok(body);
    assert_eq!(req.messages.len(), 1);
    match req.messages[0].content.first() {
        Some(CanonicalContent::Thinking { text, .. }) => assert_eq!(text, "a\nb"),
        other => panic!("expected thinking, got {other:?}"),
    }
}

#[test]
fn parse_reasoning_item_empty_summary_dropped() {
    let body = br#"{
        "model":"gpt-4o",
        "input":[{"type":"reasoning","summary":[]}]
    }"#;
    let req = parse_ok(body);
    assert_eq!(req.messages.len(), 0);
}

#[test]
fn parse_tools_function_type_only() {
    let body = br#"{
        "model":"gpt-4o",
        "tools":[
            {"type":"function","name":"search","description":"web","parameters":{"type":"object"}},
            {"type":"computer_use_preview"},
            {"type":"function","name":"calc"}
        ]
    }"#;
    let req = parse_ok(body);
    assert_eq!(req.tools.len(), 2);
    assert_eq!(req.tools[0].name, "search");
    assert_eq!(req.tools[1].name, "calc");
}

#[test]
fn parse_tool_choice_string_auto() {
    let req = parse_ok(br#"{"model":"gpt-4o","tool_choice":"auto"}"#);
    assert!(matches!(req.tool_choice, Some(CanonicalToolChoice::Auto)));
}

#[test]
fn parse_tool_choice_string_none() {
    let req = parse_ok(br#"{"model":"gpt-4o","tool_choice":"none"}"#);
    assert!(matches!(req.tool_choice, Some(CanonicalToolChoice::None)));
}

#[test]
fn parse_tool_choice_string_required() {
    let req = parse_ok(br#"{"model":"gpt-4o","tool_choice":"required"}"#);
    assert!(matches!(
        req.tool_choice,
        Some(CanonicalToolChoice::Required)
    ));
}

#[test]
fn parse_tool_choice_function_object() {
    let req = parse_ok(br#"{"model":"gpt-4o","tool_choice":{"type":"function","name":"search"}}"#);
    match req.tool_choice {
        Some(CanonicalToolChoice::Tool(n)) => assert_eq!(n, "search"),
        other => panic!("expected Tool, got {other:?}"),
    }
}

#[test]
fn parse_tool_choice_unknown_string_returns_none() {
    let req = parse_ok(br#"{"model":"gpt-4o","tool_choice":"xyz"}"#);
    assert!(req.tool_choice.is_none());
}

#[test]
fn parse_reasoning_effort_low_medium_high() {
    for (eff, exp_budget) in [("low", 1024u32), ("medium", 4096), ("high", 16384)] {
        let body = format!(r#"{{"model":"gpt-4o","reasoning":{{"effort":"{eff}"}}}}"#);
        let req = parse_ok(body.as_bytes());
        let t = req.thinking.expect("thinking present");
        assert!(t.enabled);
        assert_eq!(t.budget_tokens, Some(exp_budget));
    }
}

#[test]
fn parse_reasoning_empty_effort_disabled() {
    let req = parse_ok(br#"{"model":"gpt-4o","reasoning":{"effort":""}}"#);
    let t = req.thinking.expect("thinking present");
    assert!(!t.enabled);
    assert!(t.budget_tokens.is_none());
}

#[test]
fn parse_max_output_tokens_override() {
    let req = parse_ok(br#"{"model":"gpt-4o","max_output_tokens":777}"#);
    assert_eq!(req.max_tokens, 777);
}

#[test]
fn parse_temperature_top_p_round_trip() {
    let req = parse_ok(br#"{"model":"gpt-4o","temperature":0.5,"top_p":0.8}"#);
    assert_eq!(req.temperature, Some(0.5));
    assert_eq!(req.top_p, Some(0.8));
}

#[test]
fn parse_metadata_preserved() {
    let req = parse_ok(br#"{"model":"gpt-4o","metadata":{"trace":"abc"}}"#);
    assert_eq!(req.metadata.expect("meta")["trace"], "abc");
}
