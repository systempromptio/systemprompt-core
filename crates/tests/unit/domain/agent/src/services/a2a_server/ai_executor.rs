// Tests for the direct AI-provider calls in the processing pipeline:
// `process_without_tools` (streaming generation) and
// `synthesize_tool_results_with_artifacts` (post-tool summary). Both are driven
// against the in-test `StubAiProvider`, asserting accumulated text, stream
// events, and provider/model resolution from runtime + request context.

use std::sync::Arc;

use systemprompt_agent::services::SkillService;
use systemprompt_agent::services::a2a_server::processing::ai_executor::{
    SynthesizeToolResultsParams, process_without_tools, synthesize_tool_results_with_artifacts,
};
use systemprompt_agent::services::a2a_server::processing::message::StreamEvent;
use systemprompt_identifiers::{ContextId, SessionId, UserId};
use tokio::sync::mpsc;

use super::a2a_helpers::{StubAiProvider, ai_messages, request_context, runtime_info};

fn ctx() -> systemprompt_models::execution::context::RequestContext {
    request_context(
        &ContextId::generate(),
        &SessionId::generate(),
        &UserId::new("u-aiexec"),
        "exec-agent",
    )
}

#[tokio::test]
async fn process_without_tools_accumulates_streamed_text() {
    let provider = Arc::new(StubAiProvider::new().with_text_stream(&["Hello, ", "world"]));
    let runtime = runtime_info("exec-agent");
    let (tx, mut rx) = mpsc::channel(32);

    let result = process_without_tools(provider, &runtime, ai_messages("hi"), tx, ctx()).await;

    let (text, tool_calls, tool_results) = result.expect("ok");
    assert_eq!(text, "Hello, world");
    assert!(tool_calls.is_empty());
    assert!(tool_results.is_empty());

    let mut events = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        events.push(ev);
    }
    // Two text chunks should have been forwarded as StreamEvent::Text.
    let text_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::Text(_)))
        .collect();
    assert_eq!(text_events.len(), 2);
}

#[tokio::test]
async fn process_without_tools_empty_stream_yields_empty_text() {
    let provider = Arc::new(StubAiProvider::new());
    let runtime = runtime_info("exec-agent");
    let (tx, _rx) = mpsc::channel(8);

    let (text, _, _) = process_without_tools(provider, &runtime, ai_messages("hi"), tx, ctx())
        .await
        .expect("ok");
    assert!(text.is_empty());
}

#[tokio::test]
async fn process_without_tools_stream_failure_is_err_and_emits_error_event() {
    let provider = Arc::new(StubAiProvider::new().failing_stream());
    let runtime = runtime_info("exec-agent");
    let (tx, mut rx) = mpsc::channel(8);

    let result = process_without_tools(provider, &runtime, ai_messages("hi"), tx, ctx()).await;
    assert!(result.is_err());

    let mut saw_error = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, StreamEvent::Error(_)) {
            saw_error = true;
        }
    }
    assert!(saw_error, "expected an Error stream event");
}

#[tokio::test]
async fn synthesize_tool_results_returns_text_and_emits_event() {
    let provider = Arc::new(StubAiProvider::new().with_generate("Done summarizing."));
    let runtime = runtime_info("exec-agent");
    // SkillService needs a bootstrapped profile, which this unit harness does
    // not install; skip when unavailable (the path is covered in integration).
    let Ok(svc) = SkillService::new() else { return };
    let skill_service = Arc::new(svc);
    let (tx, mut rx) = mpsc::channel(8);

    let synthesized = synthesize_tool_results_with_artifacts(SynthesizeToolResultsParams {
        ai_service: provider,
        agent_runtime: &runtime,
        original_messages: ai_messages("do a thing"),
        initial_response: "initial",
        tool_calls: &[],
        tool_results: &[],
        artifacts: &[],
        tx,
        request_context: ctx(),
        skill_service,
    })
    .await;

    assert_eq!(synthesized.expect("ok"), "Done summarizing.");

    let mut saw_text = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, StreamEvent::Text(_)) {
            saw_text = true;
        }
    }
    assert!(saw_text, "expected a Text stream event from synthesis");
}

#[test]
fn resolve_provider_config_prefers_tool_model_config() {
    use systemprompt_agent::services::a2a_server::processing::ai_executor::resolve_provider_config;
    use systemprompt_models::ai::ToolModelConfig;

    let provider = StubAiProvider::new();
    let runtime = runtime_info("exec-agent");
    let rc = ctx().with_tool_model_config(ToolModelConfig {
        provider: Some("override-provider".to_owned()),
        model: Some("override-model".to_owned()),
        max_output_tokens: Some(99),
    });

    let (p, m, t) = resolve_provider_config(&rc, &runtime, &provider);
    assert_eq!(p, "override-provider");
    assert_eq!(m, "override-model");
    assert_eq!(t, 99);
}

#[test]
fn resolve_provider_config_falls_back_to_runtime_then_defaults() {
    use systemprompt_agent::services::a2a_server::processing::ai_executor::resolve_provider_config;

    let provider = StubAiProvider::new();

    let runtime = runtime_info("exec-agent");
    let (p, m, t) = resolve_provider_config(&ctx(), &runtime, &provider);
    assert_eq!(p, "mock-provider");
    assert_eq!(m, "mock-model");
    assert_eq!(t, 1024);

    let mut bare = runtime_info("exec-agent");
    bare.provider = None;
    bare.model = None;
    bare.max_output_tokens = None;
    let (p, m, t) = resolve_provider_config(&ctx(), &bare, &provider);
    assert_eq!(p, "mock-provider");
    assert_eq!(m, "mock-model");
    assert_eq!(t, 4096);
}

#[test]
fn resolve_provider_config_partial_tool_config_mixes_sources() {
    use systemprompt_agent::services::a2a_server::processing::ai_executor::resolve_provider_config;
    use systemprompt_models::ai::ToolModelConfig;

    let provider = StubAiProvider::new();
    let runtime = runtime_info("exec-agent");
    let rc = ctx().with_tool_model_config(ToolModelConfig {
        provider: None,
        model: Some("only-model".to_owned()),
        max_output_tokens: None,
    });

    let (p, m, t) = resolve_provider_config(&rc, &runtime, &provider);
    assert_eq!(p, "mock-provider");
    assert_eq!(m, "only-model");
    assert_eq!(t, 1024);
}
