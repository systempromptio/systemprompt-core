//! Behaviour tests for the provider-agnostic AI request/response types:
//! `AiContentPart`/`AiMessage` constructors, `AiRequestBuilder`, `AiResponse`
//! builders and serde skips, `McpTool` builders, and `TemplateResolver`
//! nested-path resolution.

use serde_json::{Value, json};
use systemprompt_identifiers::{AgentName, ContextId, McpServerId, SessionId, TraceId};
use systemprompt_models::ai::execution_plan::ToolCallResult;
use systemprompt_models::ai::{
    AiContentPart, AiMessage, AiRequest, AiResponse, McpTool, MessageRole, SamplingParams,
    StructuredOutputOptions, TemplateResolver, ToolCall, ToolModelConfig,
};
use systemprompt_models::execution::context::RequestContext;
use uuid::Uuid;

fn request_context() -> RequestContext {
    RequestContext::new(
        SessionId::new("sess-air"),
        TraceId::new("trace-air"),
        ContextId::new("00000000-0000-4000-8000-0000000000aa"),
        AgentName::new("air-agent"),
    )
}

#[test]
fn content_part_constructors_set_variant_and_media_flag() {
    let text = AiContentPart::text("hi");
    let image = AiContentPart::image("image/png", "aGk=");
    let audio = AiContentPart::audio("audio/wav", "aGk=");
    let video = AiContentPart::video("video/mp4", "aGk=");

    assert!(!text.is_media());
    assert!(image.is_media());
    assert!(audio.is_media());
    assert!(video.is_media());

    assert_eq!(
        serde_json::to_value(&image).expect("serialize"),
        json!({"type": "image", "mime_type": "image/png", "data": "aGk="})
    );
    assert_eq!(
        serde_json::to_value(&audio).expect("serialize")["type"],
        "audio"
    );
    assert_eq!(
        serde_json::to_value(&video).expect("serialize")["type"],
        "video"
    );
}

#[test]
fn message_constructors_assign_roles_and_empty_parts() {
    let user = AiMessage::user("u");
    let assistant = AiMessage::assistant("a");
    let system = AiMessage::system("s");

    assert_eq!(user.role, MessageRole::User);
    assert_eq!(assistant.role, MessageRole::Assistant);
    assert_eq!(system.role, MessageRole::System);
    assert_eq!(user.content, "u");
    assert!(user.parts.is_empty());

    let wire = serde_json::to_value(&user).expect("serialize");
    assert_eq!(wire, json!({"role": "user", "content": "u"}));
}

#[test]
fn request_builder_defaults_optional_fields_to_none() {
    let request = AiRequest::builder(
        vec![AiMessage::user("hi")],
        "anthropic",
        "claude-sonnet-4-6",
        256,
        request_context(),
    )
    .build();

    assert_eq!(request.provider(), "anthropic");
    assert_eq!(request.model(), "claude-sonnet-4-6");
    assert_eq!(request.max_output_tokens(), 256);
    assert!(request.sampling.is_none());
    assert!(request.tools.is_none());
    assert!(request.structured_output.is_none());
    assert!(request.system_prompt.is_none());
    assert!(!request.has_tools());
}

#[test]
fn request_builder_carries_sampling_tools_and_structured_output() {
    let tool = McpTool::new("lookup", McpServerId::new("svc-1"));
    let request = AiRequest::builder(
        vec![AiMessage::user("hi")],
        "openai",
        "gpt-4.1",
        64,
        request_context(),
    )
    .with_sampling(SamplingParams::default())
    .with_tools(vec![tool])
    .with_structured_output(StructuredOutputOptions::default())
    .build();

    assert!(request.sampling.is_some());
    assert!(request.structured_output.is_some());
    assert!(request.has_tools());
}

#[test]
fn has_tools_is_false_for_empty_tool_list() {
    let request = AiRequest::builder(
        vec![AiMessage::user("hi")],
        "openai",
        "gpt-4.1",
        64,
        request_context(),
    )
    .with_tools(vec![])
    .build();

    assert!(!request.has_tools());
}

#[test]
fn response_builders_populate_fields() {
    let id = Uuid::new_v4();
    let response = AiResponse::new(id, "out".to_owned(), "anthropic".to_owned(), "m".to_owned())
        .with_tokens(42)
        .with_latency(17)
        .with_streaming(true)
        .with_tool_calls(vec![ToolCall {
            ai_tool_call_id: systemprompt_identifiers::AiToolCallId::new("tc-1"),
            name: "lookup".to_owned(),
            arguments: json!({}),
        }])
        .with_tool_results(vec![]);

    assert_eq!(response.request_id, id);
    assert_eq!(response.tokens_used, Some(42));
    assert_eq!(response.latency_ms, 17);
    assert!(response.is_streaming);
    assert!(response.has_tool_calls());
    assert!(!response.has_tool_results());
}

#[test]
fn response_default_omits_optional_fields_on_the_wire() {
    let wire = serde_json::to_value(AiResponse::default()).expect("serialize");
    let obj = wire.as_object().expect("object");

    assert!(!obj.contains_key("tokens_used"));
    assert!(!obj.contains_key("tool_calls"));
    assert!(!obj.contains_key("tool_results"));
    assert!(!obj.contains_key("finish_reason"));
    assert_eq!(obj["cache_hit"], json!(false));
    assert_eq!(obj["is_streaming"], json!(false));
}

#[test]
fn mcp_tool_builders_set_all_fields() {
    let tool = McpTool::new("lookup", McpServerId::new("svc-9"))
        .with_description("finds things")
        .with_input_schema(json!({"type": "object"}))
        .with_output_schema(json!({"type": "string"}))
        .with_terminal_on_success(true)
        .with_model_config(ToolModelConfig::default());

    assert_eq!(tool.name, "lookup");
    assert_eq!(tool.description.as_deref(), Some("finds things"));
    assert_eq!(tool.input_schema, Some(json!({"type": "object"})));
    assert_eq!(tool.output_schema, Some(json!({"type": "string"})));
    assert!(tool.terminal_on_success);
    assert!(tool.model_config.is_some());

    let bare = McpTool::new("bare", McpServerId::new("svc-9"));
    assert!(!bare.terminal_on_success);
    assert!(bare.description.is_none());
}

fn tool_result(output: Value) -> ToolCallResult {
    ToolCallResult {
        tool_name: "t".to_owned(),
        arguments: json!({}),
        success: true,
        output,
        error: None,
        duration_ms: 1,
    }
}

#[test]
fn template_resolver_substitutes_nested_output_path() {
    let results = vec![tool_result(json!({"user": {"name": "ada"}}))];
    let arguments = json!({
        "who": "$0.output.user.name",
        "nested": {"again": "$0.output.user.name"},
        "list": ["$0.output.user.name", 7]
    });

    let resolved = TemplateResolver::resolve_arguments(&arguments, &results);

    assert_eq!(resolved["who"], json!("ada"));
    assert_eq!(resolved["nested"]["again"], json!("ada"));
    assert_eq!(resolved["list"], json!(["ada", 7]));
}

#[test]
fn template_resolver_returns_null_for_out_of_range_index_and_missing_field() {
    let results = vec![tool_result(json!({"a": 1}))];

    let out_of_range = TemplateResolver::resolve_arguments(&json!({"v": "$5.output.a"}), &results);
    assert_eq!(out_of_range["v"], Value::Null);

    let missing_field =
        TemplateResolver::resolve_arguments(&json!({"v": "$0.output.b.c"}), &results);
    assert_eq!(missing_field["v"], Value::Null);
}

#[test]
fn template_resolver_leaves_non_template_values_untouched() {
    let results = vec![tool_result(json!({"a": 1}))];
    let arguments = json!({
        "plain": "no template here",
        "malformed": "$x.output.field",
        "number": 3,
        "flag": true,
        "nothing": null
    });

    let resolved = TemplateResolver::resolve_arguments(&arguments, &results);

    assert_eq!(resolved, arguments);
}
