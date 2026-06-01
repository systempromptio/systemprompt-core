//! Tests for the agent-side canonical bridge: per-provider auto-policy
//! (Anthropic thinking, OpenAI reasoning, OpenAI streaming temperature),
//! agent-message → canonical assembly, and canonical → agent response mapping.

use std::time::Instant;

use systemprompt_ai::models::ai::{AiContentPart, AiMessage, MessageRole, SamplingParams};
use systemprompt_ai::services::providers::canonical_bridge::{
    BridgeProvider, CanonicalBuild, to_ai_response, to_code_execution, to_search_grounded,
};
use systemprompt_models::wire::canonical::{
    CanonicalContent, CanonicalResponse, CanonicalUsage, CodeExecutionOutput, GroundedSource,
    Grounding, ImageSource,
};
use uuid::Uuid;

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
    let thinking = req.thinking.expect("thinking auto-enabled");
    assert!(thinking.enabled);
    assert_eq!(thinking.budget_tokens, Some(10240));
}

#[test]
fn anthropic_non_3_5_leaves_thinking_unset() {
    let messages = [msg(MessageRole::User, "hi")];
    let req = CanonicalBuild::new(BridgeProvider::Anthropic, &messages, "claude-3-opus", 256)
        .into_request();
    assert!(req.thinking.is_none());
}

#[test]
fn openai_o_series_requests_medium_reasoning() {
    let messages = [msg(MessageRole::User, "hi")];
    for model in ["o3", "o3-mini"] {
        let req = CanonicalBuild::new(BridgeProvider::OpenAi, &messages, model, 256).into_request();
        assert!(matches!(
            req.reasoning_effort,
            Some(systemprompt_models::wire::canonical::ReasoningEffort::Medium)
        ));
    }
}

#[test]
fn openai_chat_model_has_no_reasoning_effort() {
    let messages = [msg(MessageRole::User, "hi")];
    let req = CanonicalBuild::new(BridgeProvider::OpenAi, &messages, "gpt-4o", 256).into_request();
    assert!(req.reasoning_effort.is_none());
}

#[test]
fn openai_streaming_defaults_temperature_when_unset() {
    let messages = [msg(MessageRole::User, "hi")];
    let req = CanonicalBuild::new(BridgeProvider::OpenAi, &messages, "gpt-4o", 256)
        .with_stream(true)
        .into_request();
    assert_eq!(req.temperature, Some(0.8));
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
    assert_eq!(req.temperature, Some(0.2));
}

#[test]
fn anthropic_streaming_does_not_inject_temperature() {
    let messages = [msg(MessageRole::User, "hi")];
    let req = CanonicalBuild::new(BridgeProvider::Anthropic, &messages, "claude-3-opus", 256)
        .with_stream(true)
        .into_request();
    assert!(req.temperature.is_none());
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
    assert_eq!(req.system.as_deref(), Some("be brief\nbe kind"));
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
    assert!(matches!(content[0], CanonicalContent::Text(_)));
    assert!(matches!(
        content[1],
        CanonicalContent::Image(ImageSource::Base64 { .. })
    ));
}

fn response_with(usage: CanonicalUsage) -> CanonicalResponse {
    CanonicalResponse {
        id: "r".to_owned(),
        model: "m".to_owned(),
        content: vec![CanonicalContent::Text("answer".to_owned())],
        stop_reason: None,
        usage,
        grounding: None,
        code_execution: None,
        raw_finish_reason: Some("stop".to_owned()),
    }
}

#[test]
fn to_ai_response_maps_tokens_and_cache() {
    let usage = CanonicalUsage {
        input_tokens: 10,
        output_tokens: 5,
        cache_read_tokens: 4,
        cache_creation_tokens: 0,
        total_tokens: 15,
    };
    let response = response_with(usage);
    let ai = to_ai_response("openai", "gpt-4o", Uuid::nil(), Instant::now(), &response);
    assert_eq!(ai.content, "answer");
    assert_eq!(ai.tokens_used, Some(15));
    assert_eq!(ai.input_tokens, Some(10));
    assert!(ai.cache_hit);
    assert_eq!(ai.cache_read_tokens, Some(4));
    assert_eq!(ai.cache_creation_tokens, None);
    assert_eq!(ai.finish_reason.as_deref(), Some("stop"));
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
    assert!(exec.error.is_some());
}
