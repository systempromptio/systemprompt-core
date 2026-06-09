//! Anthropic Messages wire-codec tests.

use serde_json::{Value, json};
use systemprompt_models::wire::anthropic;
use systemprompt_models::services::ai::ModelLimits;
use systemprompt_models::wire::canonical::{
    CanonicalContent, CanonicalEvent, CanonicalMessage, CanonicalToolChoice, ContentBlockKind,
    ImageSource, ResponseFormat, Role, SearchConfig,
};

use super::{base_request, image_url, plain_tool, tool_use, tool_with_unsupported_keywords};

#[test]
fn anthropic_emits_max_tokens() {
    let body = anthropic::build_request_body(&base_request(), "upstream", None);
    assert_eq!(body["max_tokens"], json!(32));
}

#[test]
fn anthropic_clamps_max_tokens_down_to_model_cap() {
    let mut req = base_request();
    req.max_tokens = 32_000;
    let body = anthropic::build_request_body(&req, "upstream", Some(ModelLimits { max_output_tokens: 4096, ..Default::default() }));
    assert_eq!(
        body["max_tokens"],
        json!(4096),
        "max_tokens must be clamped down to the model-card cap when one is known"
    );
}

#[test]
fn anthropic_sets_system_field() {
    let mut req = base_request();
    req.system = Some("be terse".to_owned());
    let body = anthropic::build_request_body(&req, "upstream", None);
    assert_eq!(body["system"], "be terse");
}

#[test]
fn anthropic_serializes_regular_tools() {
    let mut req = base_request();
    req.tools = vec![plain_tool()];
    let body = anthropic::build_request_body(&req, "upstream", None);
    let tool = &body["tools"][0];
    assert_eq!(tool["name"], "lookup");
    assert_eq!(tool["description"], "look something up");
    assert_eq!(tool["input_schema"]["properties"]["q"]["type"], "string");
}

#[test]
fn anthropic_tool_choice_variants() {
    let cases = [
        (CanonicalToolChoice::Auto, json!({"type": "auto"})),
        (CanonicalToolChoice::Required, json!({"type": "any"})),
        (CanonicalToolChoice::Any, json!({"type": "any"})),
        (CanonicalToolChoice::None, json!({"type": "none"})),
        (
            CanonicalToolChoice::Tool("lookup".to_owned()),
            json!({"type": "tool", "name": "lookup"}),
        ),
    ];
    for (choice, expected) in cases {
        let mut req = base_request();
        req.tools = vec![plain_tool()];
        req.tool_choice = Some(choice);
        let body = anthropic::build_request_body(&req, "upstream", None);
        assert_eq!(body["tool_choice"], expected);
    }
}

#[test]
fn anthropic_renders_base64_and_url_image_blocks() {
    let mut req = base_request();
    req.messages = vec![CanonicalMessage {
        role: Role::User,
        content: vec![
            CanonicalContent::Image(ImageSource::Base64 {
                media_type: "image/png".to_owned(),
                data: "AAAA".to_owned(),
                detail: None,
            }),
            image_url("https://example.com/cat.png"),
        ],
    }];
    let body = anthropic::build_request_body(&req, "upstream", None);
    let blocks = body["messages"][0]["content"].as_array().expect("blocks");
    assert_eq!(blocks[0]["type"], "image");
    assert_eq!(blocks[0]["source"]["type"], "base64");
    assert_eq!(blocks[0]["source"]["media_type"], "image/png");
    assert_eq!(blocks[1]["source"]["type"], "url");
    assert_eq!(blocks[1]["source"]["url"], "https://example.com/cat.png");
}

#[test]
fn anthropic_tool_and_system_roles_map_to_user() {
    let mut req = base_request();
    req.messages = vec![CanonicalMessage {
        role: Role::Tool,
        content: vec![CanonicalContent::Text("result".to_owned())],
    }];
    let body = anthropic::build_request_body(&req, "upstream", None);
    assert_eq!(body["messages"][0]["role"], "user");
}

#[test]
fn anthropic_json_schema_becomes_forced_structured_output_tool() {
    let mut req = base_request();
    req.response_format = Some(ResponseFormat::JsonSchema {
        name: "structured_output".to_owned(),
        schema: json!({"type": "object"}),
        strict: true,
    });
    let body = anthropic::build_request_body(&req, "upstream", None);
    let tools = body["tools"].as_array().expect("tools array");
    assert!(tools.iter().any(|t| t["name"] == "structured_output"));
    assert_eq!(body["tool_choice"]["type"], "tool");
    assert_eq!(body["tool_choice"]["name"], "structured_output");
}

#[test]
fn anthropic_search_turn_omits_tool_choice_and_stream() {
    let mut req = base_request();
    req.stream = true;
    req.tool_choice = Some(CanonicalToolChoice::Auto);
    req.search = Some(SearchConfig {
        max_uses: Some(3),
        context_size: None,
        urls: Vec::new(),
    });
    let body = anthropic::build_request_body(&req, "upstream", None);
    let tools = body["tools"].as_array().expect("tools array");
    assert!(tools.iter().any(|t| t["name"] == "web_search"));
    assert!(body.get("tool_choice").is_none());
    assert!(body.get("stream").is_none());
}

#[test]
fn anthropic_tools_keep_supported_keywords_but_drop_schema_metadata() {
    let mut req = base_request();
    req.tools = vec![tool_with_unsupported_keywords()];
    let body = anthropic::build_request_body(&req, "upstream", None);
    let schema = &body["tools"][0]["input_schema"];
    assert!(schema.get("$schema").is_none(), "$schema metadata stripped");
    assert_eq!(schema["additionalProperties"], json!(false));
    assert!(schema.get("propertyNames").is_some());
    assert_eq!(schema["properties"]["count"]["exclusiveMinimum"], json!(0));
}

#[test]
fn anthropic_parse_derives_total_and_keeps_cache_tokens() {
    let value: Value = json!({
        "id": "msg_1",
        "model": "claude-x",
        "content": [{"type": "text", "text": "ok"}],
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 10,
            "output_tokens": 5,
            "cache_read_input_tokens": 4,
            "cache_creation_input_tokens": 1
        }
    });
    let response = anthropic::parse_response(&value, "fallback");
    assert_eq!(response.usage.input_tokens, 10);
    assert_eq!(response.usage.cache_read_tokens, 4);
    assert_eq!(response.usage.cache_creation_tokens, 1);
    assert_eq!(response.usage.total_tokens, 20);
}

#[test]
fn anthropic_sse_parses_thinking_signature_delta() {
    let frame = json!({
        "type": "content_block_delta",
        "index": 1,
        "delta": { "type": "signature_delta", "signature": "abc123==" },
    });
    match anthropic::event_from_sse(&frame, "msg_1") {
        Some(CanonicalEvent::SignatureDelta { index, signature }) => {
            assert_eq!(index, 1);
            assert_eq!(signature, "abc123==");
        },
        other => panic!("expected SignatureDelta, got {other:?}"),
    }
}

#[test]
fn anthropic_tool_use_signature_round_trips() {
    let block = anthropic::content_to_anthropic_block(&tool_use(Some("sig==")));
    assert_eq!(block["signature"], "sig==");
    let response = json!({ "content": [block] });
    let parsed = anthropic::parse_response(&response, "fallback");
    match parsed.content.first() {
        Some(CanonicalContent::ToolUse { signature, .. }) => {
            assert_eq!(signature.as_deref(), Some("sig=="));
        },
        other => panic!("expected ToolUse, got {other:?}"),
    }
}

#[test]
fn anthropic_sse_tool_use_block_start_carries_signature() {
    let frame = json!({
        "type": "content_block_start",
        "index": 3,
        "content_block": {"type": "tool_use", "id": "tu_1", "name": "lookup", "signature": "sig=="},
    });
    match anthropic::event_from_sse(&frame, "msg_1") {
        Some(CanonicalEvent::ContentBlockStart {
            block: ContentBlockKind::ToolUse { signature, .. },
            ..
        }) => assert_eq!(signature.as_deref(), Some("sig==")),
        other => panic!("expected tool_use ContentBlockStart, got {other:?}"),
    }
}
