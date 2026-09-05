//! Anthropic Messages wire-codec tests.

use serde_json::{Value, json};
use systemprompt_models::services::ai::ModelLimits;
use systemprompt_models::wire::anthropic;
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
    let body = anthropic::build_request_body(
        &req,
        "upstream",
        Some(ModelLimits {
            max_output_tokens: 4096,
            ..Default::default()
        }),
    );
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
    let tool = tools
        .iter()
        .find(|t| t["name"] == "structured_output")
        .expect("forced tool present");
    assert_eq!(tool["strict"], json!(true));
    assert_eq!(body["tool_choice"]["type"], "tool");
    assert_eq!(body["tool_choice"]["name"], "structured_output");
}

fn forced_tool_schema(schema: Value, strict: bool) -> Value {
    let mut req = base_request();
    req.response_format = Some(ResponseFormat::JsonSchema {
        name: "structured_output".to_owned(),
        schema,
        strict,
    });
    let body = anthropic::build_request_body(&req, "upstream", None);
    body["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|t| t["name"] == "structured_output"))
        .map(|t| t["input_schema"].clone())
        .expect("forced tool present")
}

#[test]
fn anthropic_strict_schema_spells_nullable_as_an_anyof_null_branch() {
    let schema = forced_tool_schema(
        json!({
            "type": "object",
            "properties": {
                "stage": {"type": ["string", "null"], "enum": ["new", "won", null]},
                "close": {"type": ["string", "null"]}
            },
            "required": ["stage", "close"],
            "additionalProperties": false
        }),
        true,
    );
    let stage = &schema["properties"]["stage"];
    assert!(stage.get("type").is_none(), "type list replaced by anyOf");
    assert_eq!(stage["anyOf"][0]["type"], "string");
    assert_eq!(
        stage["anyOf"][0]["enum"],
        json!(["new", "won"]),
        "null moves to its own branch and leaves the enum"
    );
    assert_eq!(stage["anyOf"][1], json!({"type": "null"}));
    assert_eq!(schema["properties"]["close"]["anyOf"][1]["type"], "null");
    assert_eq!(schema["required"], json!(["stage", "close"]));
}

#[test]
fn anthropic_strict_schema_closes_every_object_and_recurses() {
    let schema = forced_tool_schema(
        json!({
            "type": "object",
            "properties": {
                "tasks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {"title": {"type": "string", "maxLength": 80}},
                        "required": ["title"]
                    }
                },
                "intent": {
                    "type": "object",
                    "properties": {"confidence": {"type": "number", "minimum": 0, "maximum": 1}},
                    "required": ["confidence"]
                }
            },
            "required": ["tasks", "intent"]
        }),
        true,
    );
    assert_eq!(schema["additionalProperties"], json!(false));
    let task = &schema["properties"]["tasks"]["items"];
    assert_eq!(task["additionalProperties"], json!(false));
    assert!(task["properties"]["title"].get("maxLength").is_none());
    let confidence = &schema["properties"]["intent"]["properties"]["confidence"];
    assert!(confidence.get("minimum").is_none());
    assert!(confidence.get("maximum").is_none());
    assert_eq!(
        schema["properties"]["intent"]["additionalProperties"],
        json!(false)
    );
}

#[test]
fn anthropic_non_strict_schema_is_passed_through_unchanged() {
    let original = json!({
        "type": "object",
        "properties": {"n": {"type": ["integer", "null"], "minimum": 0}}
    });
    let schema = forced_tool_schema(original.clone(), false);
    assert_eq!(schema, original);
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
    match anthropic::events_from_sse(&frame, "msg_1")
        .into_iter()
        .next()
    {
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
    match anthropic::events_from_sse(&frame, "msg_1")
        .into_iter()
        .next()
    {
        Some(CanonicalEvent::ContentBlockStart {
            block: ContentBlockKind::ToolUse { signature, .. },
            ..
        }) => assert_eq!(signature.as_deref(), Some("sig==")),
        other => panic!("expected tool_use ContentBlockStart, got {other:?}"),
    }
}

#[test]
fn anthropic_upstream_body_strips_vendor_extension_fields() {
    let mut req = base_request();
    req.messages = vec![
        CanonicalMessage {
            role: Role::Assistant,
            content: vec![tool_use(Some("sig=="))],
        },
        CanonicalMessage {
            role: Role::Tool,
            content: vec![CanonicalContent::ToolResult {
                tool_use_id: "call_1".to_owned(),
                content: vec![CanonicalContent::Text("ok".to_owned())],
                is_error: false,
                structured_content: Some(json!({"rows": 1})),
                meta: Some(json!({"trace": "t1"})),
            }],
        },
    ];
    let body = anthropic::build_request_body(&req, "claude-x", None);
    let tool_use_block = &body["messages"][0]["content"][0];
    assert_eq!(tool_use_block["id"], "call_1");
    assert!(
        tool_use_block.get("signature").is_none(),
        "the gateway-extension `signature` key must not reach the Anthropic API"
    );
    let tool_result_block = &body["messages"][1]["content"][0];
    assert!(tool_result_block.get("structuredContent").is_none());
    assert!(tool_result_block.get("_meta").is_none());
}

#[test]
fn anthropic_upstream_body_omits_unsigned_thinking_and_empty_messages() {
    let mut req = base_request();
    req.messages = vec![
        CanonicalMessage {
            role: Role::Assistant,
            content: vec![CanonicalContent::Thinking {
                text: "unsigned".to_owned(),
                signature: None,
                id: None,
                encrypted_content: None,
            }],
        },
        CanonicalMessage {
            role: Role::Assistant,
            content: vec![
                CanonicalContent::Thinking {
                    text: "signed".to_owned(),
                    signature: Some("sig==".to_owned()),
                    id: None,
                    encrypted_content: None,
                },
                CanonicalContent::Text("answer".to_owned()),
            ],
        },
    ];
    let body = anthropic::build_request_body(&req, "claude-x", None);
    let messages = body["messages"].as_array().expect("messages");
    assert_eq!(
        messages.len(),
        1,
        "a message reduced to zero blocks must be dropped, not sent empty"
    );
    assert_eq!(messages[0]["content"][0]["type"], "thinking");
    assert_eq!(messages[0]["content"][0]["signature"], "sig==");
    assert_eq!(messages[0]["content"][1]["text"], "answer");
}

/// Anthropic's gateway contract splits inbound headers into two classes: those
/// a gateway must relay to the upstream byte-for-byte, and those it may consume
/// for routing and attribution without forwarding. These tests pin that split
/// so a new capability header is relayed by default and an identity header is
/// not.
#[test]
fn anthropic_prefixed_headers_are_forwardable() {
    for name in [
        "anthropic-beta",
        "anthropic-version",
        "Anthropic-Beta",
        "anthropic-workspace-id",
        // Why: the set grows every Claude Code release, so an unrecognised
        // `anthropic-*` name must forward rather than be dropped.
        "anthropic-not-invented-yet",
    ] {
        assert!(
            anthropic::is_forwardable_request_header(name),
            "{name} must reach the upstream unchanged"
        );
    }
}

#[test]
fn identity_headers_are_never_forwardable() {
    for name in [
        "x-claude-code-session-id",
        "x-claude-code-agent-id",
        "x-claude-code-parent-agent-id",
        "X-Stainless-Lang",
        "user-agent",
        "cookie",
        "authorization",
        "x-api-key",
        "x-systemprompt-request-id",
        "x-forwarded-for",
    ] {
        assert!(
            anthropic::is_identity_request_header(name),
            "{name} must be classified as identity"
        );
        assert!(
            !anthropic::is_forwardable_request_header(name),
            "{name} must never be relayed to a third-party provider"
        );
    }
}

#[test]
fn unrelated_headers_are_neither_forwarded_nor_recorded() {
    for name in ["content-type", "accept", "host", "content-length"] {
        assert!(!anthropic::is_forwardable_request_header(name), "{name}");
        assert!(!anthropic::is_identity_request_header(name), "{name}");
    }
}

#[test]
fn credential_headers_are_recorded_by_name_without_their_value() {
    // The gateway logs the identity vec at INFO and writes it to the audit row.
    // A live bearer token there leaks into anywhere those logs are pasted.
    let secret = "eyJhbGciOiJSUzI1NiJ9.payload.signature";
    for name in [
        "authorization",
        "Authorization",
        "proxy-authorization",
        "x-api-key",
        "cookie",
        "set-cookie",
    ] {
        assert!(
            anthropic::is_credential_request_header(name),
            "{name} must be classified as credential-bearing"
        );
        let recorded = anthropic::recordable_header_value(name, secret);
        assert_eq!(recorded, anthropic::REDACTED, "{name} must be redacted");
        assert!(
            !recorded.contains(secret),
            "{name} must not carry the credential"
        );
    }
}

#[test]
fn non_credential_identity_headers_keep_their_value() {
    // Redaction must not swallow the identity signal the audit row exists for.
    for name in [
        "user-agent",
        "x-claude-code-session-id",
        "x-stainless-os",
        "x-forwarded-for",
    ] {
        assert!(!anthropic::is_credential_request_header(name), "{name}");
        assert_eq!(anthropic::recordable_header_value(name, "value"), "value");
    }
}

// Why: this codec also fronts Anthropic-compatible upstreams, which do not all
// honour `stop_reason: "tool_use"`. An `end_turn` beside a tool_use block is
// relayed as a finished turn and the call is silently never run.
#[test]
fn anthropic_parse_reports_tool_use_even_though_the_upstream_says_end_turn() {
    use systemprompt_models::wire::canonical::CanonicalStopReason;

    let value: Value = json!({
        "id": "msg_1",
        "model": "claude-x",
        "stop_reason": "end_turn",
        "content": [{
            "type": "tool_use",
            "id": "toolu_1",
            "name": "lookup",
            "input": {"q": "rust"}
        }]
    });
    let response = anthropic::parse_response(&value, "fallback");
    assert_eq!(
        response.stop_reason,
        Some(CanonicalStopReason::ToolUse),
        "a turn carrying a tool_use block is a tool-use turn whatever the upstream calls it"
    );
    assert_eq!(
        response.raw_finish_reason.as_deref(),
        Some("end_turn"),
        "the wire's own reason must still be preserved verbatim for auditing"
    );
}

#[test]
fn anthropic_parse_keeps_max_tokens_over_a_truncated_tool_use_block() {
    use systemprompt_models::wire::canonical::CanonicalStopReason;

    let value: Value = json!({
        "id": "msg_2",
        "model": "claude-x",
        "stop_reason": "max_tokens",
        "content": [{
            "type": "tool_use",
            "id": "toolu_1",
            "name": "lookup",
            "input": {"q": "ru"}
        }]
    });
    let response = anthropic::parse_response(&value, "fallback");
    assert_eq!(
        response.stop_reason,
        Some(CanonicalStopReason::MaxTokens),
        "a truncated tool call must report the cutoff rather than claim to be runnable"
    );
}

#[test]
fn anthropic_parse_reports_no_separate_reasoning_count() {
    let value: Value = json!({
        "id": "msg_1",
        "model": "claude-sonnet-5",
        "content": [
            {"type": "thinking", "thinking": "let me work through it", "signature": "sig"},
            {"type": "text", "text": "42"}
        ],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 10, "output_tokens": 300}
    });
    let response = anthropic::parse_response(&value, "fallback");
    assert_eq!(
        response.usage.reasoning_tokens, 0,
        "Anthropic bills extended thinking as ordinary output tokens and reports no separate \
         count, so the thinking spend is already inside output_tokens"
    );
    assert_eq!(response.usage.output_tokens, 300);
}
