//! Deeper parse-branch coverage for the Anthropic Messages inbound adapter:
//! content blocks (image / tool_use / tool_result / thinking), tools and
//! tool_choice variants, optional sampling params, and system-as-array.

use bytes::Bytes;
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalToolChoice, ImageSource, Role,
};
use systemprompt_api::services::gateway::protocol::inbound::InboundAdapter;
use systemprompt_api::services::gateway::protocol::inbound::InboundParseError;
use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;

fn parse_ok(body: &[u8]) -> systemprompt_api::services::gateway::protocol::canonical::CanonicalRequest {
    let a = AnthropicMessagesInbound;
    a.parse_request(&Bytes::copy_from_slice(body)).expect("parse")
}

#[test]
fn parses_system_array_joins_text_blocks() {
    let body = br#"{
        "model":"m","max_tokens":1,
        "system":[{"type":"text","text":"a"},{"type":"text","text":"b"}],
        "messages":[{"role":"user","content":"hi"}]
    }"#;
    let req = parse_ok(body);
    assert_eq!(req.system.as_deref(), Some("a\nb"));
}

#[test]
fn parses_empty_system_array_is_none() {
    let body = br#"{
        "model":"m","max_tokens":1,"system":[],
        "messages":[{"role":"user","content":"hi"}]
    }"#;
    let req = parse_ok(body);
    assert!(req.system.is_none());
}

#[test]
fn parses_null_system_is_none() {
    let body = br#"{
        "model":"m","max_tokens":1,"system":null,
        "messages":[{"role":"user","content":"hi"}]
    }"#;
    let req = parse_ok(body);
    assert!(req.system.is_none());
}

#[test]
fn parses_empty_system_string_is_none() {
    let body = br#"{
        "model":"m","max_tokens":1,"system":"",
        "messages":[{"role":"user","content":"hi"}]
    }"#;
    let req = parse_ok(body);
    assert!(req.system.is_none());
}

#[test]
fn parses_all_roles() {
    let body = br#"{
        "model":"m","max_tokens":1,
        "messages":[
            {"role":"user","content":"u"},
            {"role":"assistant","content":"a"},
            {"role":"system","content":"s"},
            {"role":"tool","content":"t"}
        ]
    }"#;
    let req = parse_ok(body);
    assert_eq!(req.messages.len(), 4);
    assert_eq!(req.messages[0].role, Role::User);
    assert_eq!(req.messages[1].role, Role::Assistant);
    assert_eq!(req.messages[2].role, Role::System);
    assert_eq!(req.messages[3].role, Role::Tool);
}

#[test]
fn parse_unknown_role_returns_unsupported() {
    let a = AnthropicMessagesInbound;
    let body = br#"{
        "model":"m","max_tokens":1,
        "messages":[{"role":"narrator","content":"x"}]
    }"#;
    let err = a.parse_request(&Bytes::from_static(body)).expect_err("err");
    assert!(matches!(err, InboundParseError::Unsupported { field, .. } if field == "messages[].role"));
}

#[test]
fn parse_missing_message_role_errors() {
    let a = AnthropicMessagesInbound;
    let body = br#"{
        "model":"m","max_tokens":1,
        "messages":[{"content":"x"}]
    }"#;
    let err = a.parse_request(&Bytes::from_static(body)).expect_err("err");
    assert!(matches!(err, InboundParseError::MissingField("messages[].role")));
}

#[test]
fn parse_missing_message_content_errors() {
    let a = AnthropicMessagesInbound;
    let body = br#"{
        "model":"m","max_tokens":1,
        "messages":[{"role":"user"}]
    }"#;
    let err = a.parse_request(&Bytes::from_static(body)).expect_err("err");
    assert!(matches!(err, InboundParseError::MissingField("messages[].content")));
}

#[test]
fn parse_content_block_text() {
    let body = br#"{
        "model":"m","max_tokens":1,
        "messages":[{"role":"user","content":[{"type":"text","text":"hello"}]}]
    }"#;
    let req = parse_ok(body);
    match req.messages[0].content.first() {
        Some(CanonicalContent::Text(t)) => assert_eq!(t, "hello"),
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn parse_content_block_image_base64() {
    let body = br#"{
        "model":"m","max_tokens":1,
        "messages":[{"role":"user","content":[
            {"type":"image","source":{"type":"base64","media_type":"image/png","data":"AAA="}}
        ]}]
    }"#;
    let req = parse_ok(body);
    match req.messages[0].content.first() {
        Some(CanonicalContent::Image(ImageSource::Base64 { media_type, data })) => {
            assert_eq!(media_type, "image/png");
            assert_eq!(data, "AAA=");
        },
        other => panic!("expected base64 image, got {other:?}"),
    }
}

#[test]
fn parse_content_block_image_url() {
    let body = br#"{
        "model":"m","max_tokens":1,
        "messages":[{"role":"user","content":[
            {"type":"image","source":{"type":"url","url":"https://example.com/x.png"}}
        ]}]
    }"#;
    let req = parse_ok(body);
    match req.messages[0].content.first() {
        Some(CanonicalContent::Image(ImageSource::Url(u))) => {
            assert_eq!(u, "https://example.com/x.png");
        },
        other => panic!("expected url image, got {other:?}"),
    }
}

#[test]
fn parse_content_block_tool_use() {
    let body = br#"{
        "model":"m","max_tokens":1,
        "messages":[{"role":"assistant","content":[
            {"type":"tool_use","id":"tu_1","name":"search","input":{"q":"rust"}}
        ]}]
    }"#;
    let req = parse_ok(body);
    match req.messages[0].content.first() {
        Some(CanonicalContent::ToolUse { id, name, input }) => {
            assert_eq!(id, "tu_1");
            assert_eq!(name, "search");
            assert_eq!(input["q"], "rust");
        },
        other => panic!("expected tool_use, got {other:?}"),
    }
}

#[test]
fn parse_content_block_tool_result_text_content() {
    let body = br#"{
        "model":"m","max_tokens":1,
        "messages":[{"role":"user","content":[
            {"type":"tool_result","tool_use_id":"tu_1","content":[{"type":"text","text":"42"}],"is_error":false}
        ]}]
    }"#;
    let req = parse_ok(body);
    match req.messages[0].content.first() {
        Some(CanonicalContent::ToolResult { tool_use_id, content, is_error }) => {
            assert_eq!(tool_use_id, "tu_1");
            assert!(!is_error);
            assert!(matches!(content.first(), Some(CanonicalContent::Text(t)) if t == "42"));
        },
        other => panic!("expected tool_result, got {other:?}"),
    }
}

#[test]
fn parse_content_block_tool_result_error_string() {
    let body = br#"{
        "model":"m","max_tokens":1,
        "messages":[{"role":"user","content":[
            {"type":"tool_result","tool_use_id":"tu_2","content":"oops","is_error":true}
        ]}]
    }"#;
    let req = parse_ok(body);
    match req.messages[0].content.first() {
        Some(CanonicalContent::ToolResult { tool_use_id, is_error, .. }) => {
            assert_eq!(tool_use_id, "tu_2");
            assert!(is_error);
        },
        other => panic!("expected tool_result, got {other:?}"),
    }
}

#[test]
fn parse_content_block_thinking() {
    let body = br#"{
        "model":"m","max_tokens":1,
        "messages":[{"role":"assistant","content":[
            {"type":"thinking","thinking":"hmm","signature":"sig123"}
        ]}]
    }"#;
    let req = parse_ok(body);
    match req.messages[0].content.first() {
        Some(CanonicalContent::Thinking { text, signature }) => {
            assert_eq!(text, "hmm");
            assert_eq!(signature.as_deref(), Some("sig123"));
        },
        other => panic!("expected thinking, got {other:?}"),
    }
}

#[test]
fn parse_unknown_content_block_returns_unsupported() {
    let a = AnthropicMessagesInbound;
    let body = br#"{
        "model":"m","max_tokens":1,
        "messages":[{"role":"user","content":[{"type":"audio","data":"xxx"}]}]
    }"#;
    let err = a.parse_request(&Bytes::from_static(body)).expect_err("err");
    assert!(matches!(err, InboundParseError::Unsupported { field, .. } if field == "messages[].content[].type"));
}

#[test]
fn parse_optional_sampling_params() {
    let body = br#"{
        "model":"m","max_tokens":4,
        "temperature":0.7,"top_p":0.9,"top_k":40,
        "stop_sequences":["END","STOP"],
        "messages":[{"role":"user","content":"x"}]
    }"#;
    let req = parse_ok(body);
    assert_eq!(req.temperature, Some(0.7));
    assert_eq!(req.top_p, Some(0.9));
    assert_eq!(req.top_k, Some(40));
    assert_eq!(req.stop_sequences, vec!["END".to_owned(), "STOP".to_owned()]);
}

#[test]
fn parse_tools_array() {
    let body = br#"{
        "model":"m","max_tokens":1,
        "tools":[
            {"name":"search","description":"web","input_schema":{"type":"object"}},
            {"name":"calc","input_schema":{"type":"object"}}
        ],
        "messages":[{"role":"user","content":"x"}]
    }"#;
    let req = parse_ok(body);
    assert_eq!(req.tools.len(), 2);
    assert_eq!(req.tools[0].name, "search");
    assert_eq!(req.tools[0].description.as_deref(), Some("web"));
    assert_eq!(req.tools[1].name, "calc");
    assert!(req.tools[1].description.is_none());
}

#[test]
fn parse_tool_choice_auto() {
    let body = br#"{"model":"m","max_tokens":1,"tool_choice":{"type":"auto"},"messages":[{"role":"user","content":"x"}]}"#;
    let req = parse_ok(body);
    assert!(matches!(req.tool_choice, Some(CanonicalToolChoice::Auto)));
}

#[test]
fn parse_tool_choice_any() {
    let body = br#"{"model":"m","max_tokens":1,"tool_choice":{"type":"any"},"messages":[{"role":"user","content":"x"}]}"#;
    let req = parse_ok(body);
    assert!(matches!(req.tool_choice, Some(CanonicalToolChoice::Any)));
}

#[test]
fn parse_tool_choice_named() {
    let body = br#"{"model":"m","max_tokens":1,"tool_choice":{"type":"tool","name":"search"},"messages":[{"role":"user","content":"x"}]}"#;
    let req = parse_ok(body);
    match req.tool_choice {
        Some(CanonicalToolChoice::Tool(name)) => assert_eq!(name, "search"),
        other => panic!("expected Tool, got {other:?}"),
    }
}

#[test]
fn parse_thinking_enabled() {
    let body = br#"{
        "model":"m","max_tokens":1,
        "thinking":{"type":"enabled","budget_tokens":1024},
        "messages":[{"role":"user","content":"x"}]
    }"#;
    let req = parse_ok(body);
    let t = req.thinking.expect("thinking present");
    assert!(t.enabled);
    assert_eq!(t.budget_tokens, Some(1024));
}

#[test]
fn parse_metadata_is_preserved() {
    let body = br#"{
        "model":"m","max_tokens":1,
        "metadata":{"user_id":"u1","trace":"abc"},
        "messages":[{"role":"user","content":"x"}]
    }"#;
    let req = parse_ok(body);
    let meta = req.metadata.expect("metadata present");
    assert_eq!(meta["user_id"], "u1");
    assert_eq!(meta["trace"], "abc");
}
