//! Tests for the agent-side canonical bridge: per-provider auto-policy
//! (Anthropic thinking, OpenAI reasoning, OpenAI streaming temperature),
//! agent-message → canonical assembly, and canonical → agent response mapping.

use std::time::Instant;

use serde_json::json;
use systemprompt_ai::models::ai::{
    AiContentPart, AiMessage, MessageRole, ReasoningEffort, ReasoningParams,
    ResponseFormat as AgentResponseFormat, SamplingParams, StreamChunk,
};
use systemprompt_ai::models::tools::McpTool;
use systemprompt_ai::services::providers::canonical_bridge::{
    BridgeProvider, CanonicalBuild, agent_response_format, event_to_chunk, finish_reason,
    text_content, to_ai_response, to_code_execution, to_search_grounded, tool_calls,
    tools_to_canonical,
};
use systemprompt_identifiers::{AiRequestId, McpServerId, ToolUseId};
use systemprompt_wire::canonical::{
    CanonicalContent, CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage,
    CanonicalUsageUpdate, CodeExecutionOutput, GroundedSource, Grounding, ImageSource,
    ResponseFormat,
};

fn msg(role: MessageRole, content: &str) -> AiMessage {
    AiMessage {
        role,
        content: content.to_owned(),
        parts: Vec::new(),
    }
}

#[test]
fn anthropic_claude_3_5_enables_extended_thinking() {
    let messages = [msg(MessageRole::User, "hi")];
    let req = CanonicalBuild::new(
        BridgeProvider::Anthropic,
        &messages,
        "claude-3-5-sonnet",
        256,
    )
    .into_request();
    let thinking = req.reasoning.thinking.expect("thinking auto-enabled");
    assert!(thinking.enabled);
    assert_eq!(thinking.budget_tokens, Some(10240));
}

#[test]
fn anthropic_non_3_5_leaves_thinking_unset() {
    let messages = [msg(MessageRole::User, "hi")];
    let req = CanonicalBuild::new(BridgeProvider::Anthropic, &messages, "claude-3-opus", 256)
        .into_request();
    assert!(req.reasoning.thinking.is_none());
}

#[test]
fn openai_o_series_requests_medium_reasoning() {
    let messages = [msg(MessageRole::User, "hi")];
    for model in ["o3", "o3-mini"] {
        let req = CanonicalBuild::new(BridgeProvider::OpenAi, &messages, model, 256).into_request();
        assert!(matches!(
            req.reasoning.reasoning_effort,
            Some(systemprompt_wire::canonical::ReasoningEffort::Medium)
        ));
    }
}

#[test]
fn openai_chat_model_has_no_reasoning_effort() {
    let messages = [msg(MessageRole::User, "hi")];
    let req = CanonicalBuild::new(BridgeProvider::OpenAi, &messages, "gpt-4o", 256).into_request();
    assert!(req.reasoning.reasoning_effort.is_none());
}

#[test]
fn openai_streaming_defaults_temperature_when_unset() {
    let messages = [msg(MessageRole::User, "hi")];
    let req = CanonicalBuild::new(BridgeProvider::OpenAi, &messages, "gpt-4o", 256)
        .with_stream(true)
        .into_request();
    assert_eq!(req.sampling.temperature, Some(0.8));
}

#[test]
fn explicit_temperature_overrides_streaming_default() {
    let messages = [msg(MessageRole::User, "hi")];
    let sampling = SamplingParams {
        temperature: Some(0.2),
        ..SamplingParams::default()
    };
    let req = CanonicalBuild::new(BridgeProvider::OpenAi, &messages, "gpt-4o", 256)
        .with_sampling(Some(&sampling))
        .with_stream(true)
        .into_request();
    assert_eq!(req.sampling.temperature, Some(0.2));
}

#[test]
fn anthropic_streaming_does_not_inject_temperature() {
    let messages = [msg(MessageRole::User, "hi")];
    let req = CanonicalBuild::new(BridgeProvider::Anthropic, &messages, "claude-3-opus", 256)
        .with_stream(true)
        .into_request();
    assert!(req.sampling.temperature.is_none());
}

#[test]
fn system_messages_are_hoisted_and_joined() {
    let messages = [
        msg(MessageRole::System, "be brief"),
        msg(MessageRole::System, "be kind"),
        msg(MessageRole::User, "hello"),
    ];
    let req = CanonicalBuild::new(BridgeProvider::Anthropic, &messages, "claude-3-opus", 256)
        .into_request();
    assert_eq!(req.system_text().as_deref(), Some("be brief\nbe kind"));
    assert_eq!(req.messages.len(), 1);
}

#[test]
fn image_parts_become_base64_canonical_content() {
    let mut message = msg(MessageRole::User, "look");
    message.parts.push(AiContentPart::Image {
        mime_type: "image/png".to_owned(),
        data: "AAAA".to_owned(),
    });
    let req = CanonicalBuild::new(
        BridgeProvider::Gemini,
        std::slice::from_ref(&message),
        "g",
        256,
    )
    .into_request();
    let content = &req.messages[0].content;
    assert!(matches!(content[0], CanonicalContent::Text { .. }));
    assert!(matches!(
        content[1],
        CanonicalContent::Image {
            source: ImageSource::Base64 { .. },
            ..
        }
    ));
}

fn response_with(usage: CanonicalUsage) -> CanonicalResponse {
    CanonicalResponse {
        id: "r".to_owned(),
        model: "m".to_owned(),
        content: vec![CanonicalContent::text("answer".to_owned())],
        stop_reason: None,
        usage,
        grounding: None,
        code_execution: None,
        raw_finish_reason: Some("stop".to_owned()),
        ..Default::default()
    }
}

#[test]
fn to_ai_response_maps_tokens_and_cache() {
    let usage = CanonicalUsage {
        input_tokens: 10,
        output_tokens: 5,
        cache_read_tokens: 4,
        cache_creation_tokens: 0,
        reasoning_tokens: 0,
        total_tokens: 15,
    };
    let response = response_with(usage);
    let ai = to_ai_response(
        "openai",
        "gpt-4o",
        AiRequestId::generate(),
        Instant::now(),
        &response,
    );
    assert_eq!(ai.content, "answer");
    assert_eq!(ai.tokens_used, Some(19), "tokens_used counts cache reads");
    assert_eq!(ai.input_tokens, Some(10));
    assert!(ai.cache_hit);
    assert_eq!(ai.cache_read_tokens, Some(4));
    assert_eq!(ai.cache_creation_tokens, None);
    assert_eq!(ai.finish_reason.as_deref(), Some("stop"));
    assert_eq!(ai.reasoning_tokens, None);
}

#[test]
fn to_ai_response_carries_reasoning_without_adding_it_to_tokens_used() {
    let usage = CanonicalUsage {
        input_tokens: 20,
        output_tokens: 106,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
        reasoning_tokens: 100,
        total_tokens: 126,
    };
    let ai = to_ai_response(
        "openai",
        "o4-mini",
        AiRequestId::generate(),
        Instant::now(),
        &response_with(usage),
    );
    assert_eq!(ai.reasoning_tokens, Some(100));
    assert_eq!(ai.output_tokens, Some(106));
    assert_eq!(ai.tokens_used, Some(126));
}

#[test]
fn to_ai_response_folds_reasoning_a_provider_reported_additionally() {
    let usage = CanonicalUsage {
        input_tokens: 20,
        output_tokens: 40,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
        reasoning_tokens: 100,
        total_tokens: 0,
    };
    let ai = to_ai_response(
        "cerebras",
        "gpt-oss-120b",
        AiRequestId::generate(),
        Instant::now(),
        &response_with(usage),
    );
    assert_eq!(ai.output_tokens, Some(140));
    assert_eq!(ai.tokens_used, Some(160));
}

#[test]
fn to_search_grounded_collects_sources_and_queries() {
    let mut response = response_with(CanonicalUsage::default());
    response.grounding = Some(Grounding {
        sources: vec![GroundedSource {
            uri: "https://example.com".to_owned(),
            title: Some("Example".to_owned()),
            snippet: None,
            relevance: Some(0.9),
        }],
        queries: vec!["rust async".to_owned()],
    });
    let grounded = to_search_grounded(Instant::now(), &response);
    assert_eq!(grounded.sources.len(), 1);
    assert_eq!(grounded.sources[0].uri, "https://example.com");
    assert_eq!(grounded.confidence_scores, vec![0.9]);
    assert_eq!(grounded.web_search_queries, vec!["rust async".to_owned()]);
}

#[test]
fn to_code_execution_marks_success_on_outcome_ok() {
    let mut response = response_with(CanonicalUsage::default());
    response.code_execution = Some(CodeExecutionOutput {
        language: Some("python".to_owned()),
        code: "print(1)".to_owned(),
        result: Some("1".to_owned()),
        outcome: Some("OUTCOME_OK".to_owned()),
    });
    let exec = to_code_execution(Instant::now(), &response);
    assert!(exec.success);
    assert_eq!(exec.generated_code, "print(1)");
    assert_eq!(exec.execution_output, "1");
    assert!(exec.error.is_none());
}

#[test]
fn to_code_execution_reports_failure_outcome() {
    let mut response = response_with(CanonicalUsage::default());
    response.code_execution = Some(CodeExecutionOutput {
        language: None,
        code: "boom".to_owned(),
        result: None,
        outcome: Some("OUTCOME_FAILED".to_owned()),
    });
    let exec = to_code_execution(Instant::now(), &response);
    assert!(!exec.success);
    assert_eq!(
        exec.error.as_deref(),
        Some("Code execution failed: OUTCOME_FAILED")
    );
}

#[test]
fn text_content_concatenates_only_text_parts() {
    let response = CanonicalResponse {
        id: "r".to_owned(),
        model: "m".to_owned(),
        content: vec![
            CanonicalContent::text("hello ".to_owned()),
            CanonicalContent::ToolUse {
                id: "t1".to_owned(),
                name: "search".to_owned(),
                input: json!({}),
                signature: None,
                cache_control: None,
            },
            CanonicalContent::text("world".to_owned()),
        ],
        stop_reason: None,
        usage: CanonicalUsage::default(),
        grounding: None,
        code_execution: None,
        raw_finish_reason: None,
        ..Default::default()
    };
    assert_eq!(text_content(&response), "hello world");
}

#[test]
fn tool_calls_extracts_tool_use_parts() {
    let response = CanonicalResponse {
        id: "r".to_owned(),
        model: "m".to_owned(),
        content: vec![
            CanonicalContent::text("ignored".to_owned()),
            CanonicalContent::ToolUse {
                id: "call-1".to_owned(),
                name: "lookup".to_owned(),
                input: json!({"q": "x"}),
                signature: None,
                cache_control: None,
            },
        ],
        stop_reason: None,
        usage: CanonicalUsage::default(),
        grounding: None,
        code_execution: None,
        raw_finish_reason: None,
        ..Default::default()
    };
    let calls = tool_calls(&response);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "lookup");
    assert_eq!(calls[0].ai_tool_call_id.as_str(), "call-1");
    assert_eq!(calls[0].arguments, json!({"q": "x"}));
}

#[test]
fn agent_response_format_text_maps_to_none() {
    assert!(agent_response_format(&AgentResponseFormat::Text).is_none());
}

#[test]
fn agent_response_format_json_object_maps_through() {
    assert!(matches!(
        agent_response_format(&AgentResponseFormat::JsonObject),
        Some(ResponseFormat::JsonObject)
    ));
}

#[test]
fn agent_response_format_json_schema_defaults_name_and_strict() {
    let format = AgentResponseFormat::JsonSchema {
        schema: json!({"type": "object"}),
        name: None,
        strict: None,
    };
    match agent_response_format(&format) {
        Some(ResponseFormat::JsonSchema { name, strict, .. }) => {
            assert_eq!(name, "structured_output");
            assert!(strict);
        },
        other => panic!("expected JsonSchema, got {other:?}"),
    }
}

#[test]
fn tools_to_canonical_fills_empty_schema_default() {
    let tools = vec![
        McpTool::new(
            "with_schema",
            McpServerId::try_new("svc").expect("valid McpServerId"),
        ),
        {
            let mut t = McpTool::new(
                "with_schema",
                McpServerId::try_new("svc").expect("valid McpServerId"),
            );
            t.input_schema = Some(json!({"type": "object", "properties": {"a": {}}}));
            t
        },
    ];
    let canonical = tools_to_canonical(tools);
    assert_eq!(canonical.len(), 2);
    // First tool had no schema, so it gets the empty-object default.
    assert_eq!(
        canonical[0].input_schema,
        json!({"type": "object", "properties": {}})
    );
    assert_eq!(
        canonical[1].input_schema,
        json!({"type": "object", "properties": {"a": {}}})
    );
}

#[test]
fn event_to_chunk_text_delta_becomes_text() {
    let chunk = event_to_chunk(CanonicalEvent::TextDelta {
        index: 0,
        text: "hi".to_owned(),
    });
    assert!(matches!(chunk, Some(StreamChunk::Text(t)) if t == "hi"));
}

#[test]
fn event_to_chunk_empty_text_delta_is_dropped() {
    let chunk = event_to_chunk(CanonicalEvent::TextDelta {
        index: 0,
        text: String::new(),
    });
    assert!(chunk.is_none());
}

#[test]
fn stop_reasons_map_to_the_recorded_finish_reason() {
    assert_eq!(finish_reason(CanonicalStopReason::MaxTokens), "length");
    assert_eq!(finish_reason(CanonicalStopReason::ToolUse), "tool_calls");
    assert_eq!(finish_reason(CanonicalStopReason::EndTurn), "stop");
    assert_eq!(
        finish_reason(CanonicalStopReason::Refusal),
        "content_filter"
    );
}

#[test]
fn event_to_chunk_projects_text_only() {
    let usage = CanonicalUsageUpdate {
        input_tokens: Some(12),
        output_tokens: Some(8),
        cache_read_tokens: Some(3),
        cache_creation_tokens: None,
        reasoning_tokens: None,
        total_tokens: Some(23),
    };
    assert!(event_to_chunk(CanonicalEvent::UsageDelta(usage)).is_none());
    assert!(
        event_to_chunk(CanonicalEvent::MessageStop {
            id: "m".to_owned(),
            stop_reason: Some(CanonicalStopReason::EndTurn),
            raw_finish_reason: None,
        })
        .is_none()
    );
    assert!(
        event_to_chunk(CanonicalEvent::ThinkingDelta {
            index: 0,
            text: "pondering".to_owned(),
        })
        .is_none()
    );
    assert!(event_to_chunk(CanonicalEvent::Error("boom".to_owned())).is_none());
    assert!(event_to_chunk(CanonicalEvent::ContentBlockStop { index: 0 }).is_none());
}

#[test]
fn explicit_thinking_budget_overrides_the_model_heuristic() {
    let messages = [msg(MessageRole::User, "hi")];
    let request = CanonicalBuild::new(
        BridgeProvider::Anthropic,
        &messages,
        "claude-sonnet-5",
        4096,
    )
    .with_reasoning(Some(
        ReasoningParams::effort(ReasoningEffort::High).with_thinking_budget(2048),
    ))
    .into_request();
    let thinking = request.reasoning.thinking.expect("thinking requested");
    assert!(thinking.enabled);
    assert_eq!(thinking.budget_tokens, Some(2048));
    assert_eq!(
        request.reasoning.reasoning_effort,
        Some(systemprompt_wire::canonical::ReasoningEffort::High)
    );
}

#[test]
fn explicit_effort_replaces_the_o_series_default() {
    let messages = [msg(MessageRole::User, "hi")];
    let request = CanonicalBuild::new(BridgeProvider::OpenAi, &messages, "o3-mini", 4096)
        .with_reasoning(Some(ReasoningParams::effort(ReasoningEffort::Low)))
        .into_request();
    assert_eq!(
        request.reasoning.reasoning_effort,
        Some(systemprompt_wire::canonical::ReasoningEffort::Low)
    );
    assert!(request.reasoning.thinking.is_none());
}

#[test]
fn thinking_and_tool_results_lead_a_replayed_turn_that_also_has_text() {
    let assistant = AiMessage {
        role: MessageRole::Assistant,
        content: "let me look".to_owned(),
        parts: vec![
            AiContentPart::tool_use("call-1", "search", json!({"q": "rust"})),
            AiContentPart::thinking("weighing options", Some("sig-1".to_owned())),
        ],
    };
    let user = AiMessage {
        role: MessageRole::User,
        content: "and summarise it".to_owned(),
        parts: vec![AiContentPart::tool_result(
            ToolUseId::new("call-1"),
            "3 hits",
            false,
        )],
    };
    let messages = [msg(MessageRole::User, "find it"), assistant, user];
    let request = CanonicalBuild::new(BridgeProvider::Anthropic, &messages, "claude-sonnet-5", 512)
        .into_request();

    assert!(
        matches!(
            request.messages[1].content.as_slice(),
            [
                CanonicalContent::Thinking { .. },
                CanonicalContent::Text { .. },
                CanonicalContent::ToolUse { .. },
            ]
        ),
        "an assistant turn opens with its thinking and closes with its tool use: {:?}",
        request.messages[1].content
    );
    assert!(
        matches!(
            request.messages[2].content.as_slice(),
            [
                CanonicalContent::ToolResult { .. },
                CanonicalContent::Text { .. },
            ]
        ),
        "a user turn opens with its tool results: {:?}",
        request.messages[2].content
    );
}

#[test]
fn tool_and_thinking_parts_become_canonical_blocks() {
    let assistant = AiMessage {
        role: MessageRole::Assistant,
        content: String::new(),
        parts: vec![
            AiContentPart::thinking("weighing options", Some("sig-1".to_owned())),
            AiContentPart::tool_use("call-1", "search", json!({"q": "rust"})),
        ],
    };
    let user = AiMessage {
        role: MessageRole::User,
        content: String::new(),
        parts: vec![AiContentPart::tool_result(
            ToolUseId::new("call-1"),
            "3 hits",
            false,
        )],
    };
    let messages = [msg(MessageRole::User, "find it"), assistant, user];
    let request = CanonicalBuild::new(BridgeProvider::Anthropic, &messages, "claude-sonnet-5", 512)
        .into_request();

    assert_eq!(request.messages.len(), 3);
    match request.messages[1].content.as_slice() {
        [
            CanonicalContent::Thinking {
                text, signature, ..
            },
            CanonicalContent::ToolUse {
                id, name, input, ..
            },
        ] => {
            assert_eq!(text, "weighing options");
            assert_eq!(signature.as_deref(), Some("sig-1"));
            assert_eq!(id, "call-1");
            assert_eq!(name, "search");
            assert_eq!(input, &json!({"q": "rust"}));
        },
        other => panic!("expected thinking then tool use, got {other:?}"),
    }
    match request.messages[2].content.as_slice() {
        [
            CanonicalContent::ToolResult {
                tool_use_id,
                content,
                is_error,
                ..
            },
        ] => {
            assert_eq!(tool_use_id, "call-1");
            assert!(!is_error);
            assert!(
                matches!(content.as_slice(), [CanonicalContent::Text { text, .. }] if text == "3 hits")
            );
        },
        other => panic!("expected one tool result, got {other:?}"),
    }
}

fn tool_turn() -> [AiMessage; 3] {
    [
        msg(MessageRole::User, "find it"),
        AiMessage {
            role: MessageRole::Assistant,
            content: String::new(),
            parts: vec![
                AiContentPart::thinking("weighing options", Some("sig-1".to_owned())),
                AiContentPart::tool_use("call-1", "search", json!({"q": "rust"})),
            ],
        },
        AiMessage {
            role: MessageRole::User,
            content: String::new(),
            parts: vec![AiContentPart::tool_result(
                ToolUseId::new("call-1"),
                "3 hits",
                false,
            )],
        },
    ]
}

#[test]
fn a_replayed_tool_turn_renders_natively_on_the_anthropic_wire() {
    let messages = tool_turn();
    let request = CanonicalBuild::new(BridgeProvider::Anthropic, &messages, "claude-sonnet-5", 512)
        .into_request();
    let body = systemprompt_wire::anthropic::build_request_body(&request, "claude-sonnet-5", None);

    let assistant = &body["messages"][1]["content"];
    assert_eq!(assistant[0]["type"], "thinking");
    assert_eq!(assistant[0]["signature"], "sig-1");
    assert_eq!(assistant[1]["type"], "tool_use");
    assert_eq!(assistant[1]["id"], "call-1");
    assert_eq!(assistant[1]["input"], json!({"q": "rust"}));
    let result = &body["messages"][2]["content"][0];
    assert_eq!(body["messages"][2]["role"], "user");
    assert_eq!(result["type"], "tool_result");
    assert_eq!(result["tool_use_id"], "call-1");
}

#[test]
fn a_replayed_tool_turn_renders_natively_on_the_openai_chat_wire() {
    let messages = tool_turn();
    let request =
        CanonicalBuild::new(BridgeProvider::OpenAi, &messages, "gpt-4o-mini", 512).into_request();
    let body = systemprompt_wire::openai_chat::build_request_body(&request, "gpt-4o-mini", None);

    let rendered = body["messages"].as_array().expect("messages array");
    let assistant = rendered
        .iter()
        .find(|m| m["role"] == "assistant")
        .expect("assistant message");
    assert_eq!(assistant["tool_calls"][0]["id"], "call-1");
    assert_eq!(assistant["tool_calls"][0]["function"]["name"], "search");
    let tool = rendered
        .iter()
        .find(|m| m["role"] == "tool")
        .expect("tool message");
    assert_eq!(tool["tool_call_id"], "call-1");
    assert_eq!(tool["content"], "3 hits");
}

#[test]
fn a_replayed_tool_turn_renders_natively_on_the_gemini_wire() {
    let messages = tool_turn();
    let request = CanonicalBuild::new(BridgeProvider::Gemini, &messages, "gemini-2.5-flash", 512)
        .into_request();
    let body = systemprompt_wire::gemini::build_request_body(&request, None).to_string();

    assert!(body.contains("functionCall"), "tool call part: {body}");
    assert!(
        body.contains("functionResponse"),
        "tool result part: {body}"
    );
}

#[test]
fn the_agent_path_records_the_same_tokens_used_the_gateway_would() {
    let usage = CanonicalUsage {
        input_tokens: 1_000,
        output_tokens: 500,
        cache_read_tokens: 100_000,
        cache_creation_tokens: 10_000,
        reasoning_tokens: 200,
        total_tokens: 0,
    };
    let ai = to_ai_response(
        "anthropic",
        "claude-sonnet-5",
        AiRequestId::generate(),
        Instant::now(),
        &response_with(usage),
    );
    assert_eq!(ai.tokens_used, Some(usage.billable_total()));
    assert_eq!(ai.tokens_used, Some(111_500));
}
