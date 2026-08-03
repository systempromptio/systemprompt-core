// The `AiProvider` trait impl on `AiService` — the seam every consumer that
// holds an `Arc<dyn AiProvider>` goes through. It is a separate surface from
// the inherent methods `ai_service.rs` exercises: each trait method re-enters
// the inherent one and maps the domain error into a boxed provider error, and
// nothing else in the suite calls it.

use std::sync::Arc;

use futures::StreamExt;
use systemprompt_ai::models::ai::{AiMessage, AiRequest};
use systemprompt_ai::models::tools::McpTool;
use systemprompt_identifiers::{AgentName, McpServerId};
use systemprompt_models::ai::{
    AiProvider, GenerateResponseParams, GoogleSearchParams, StreamChunk,
};

use super::{pool, seeded_context, service};
use crate::services::providers::mock_http;

const ANTHROPIC: &str = "anthropic";
const MODEL: &str = "claude-sonnet-4-6";

const SSE: &str = "data: {\"type\":\"message_start\",\"message\":{\"id\":\"x\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":3,\"output_tokens\":1}}}\n\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\n";

fn request(context: systemprompt_models::RequestContext) -> AiRequest {
    AiRequest::builder(
        vec![AiMessage::system("be brief"), AiMessage::user("hi")],
        ANTHROPIC,
        MODEL,
        128,
        context,
    )
    .build()
}

#[tokio::test]
async fn the_trait_reports_the_same_defaults_as_the_inherent_accessors() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("x")).await;
    let svc = service(&pool, ANTHROPIC, server.uri());

    let inherent = (
        systemprompt_ai::AiService::default_provider(&svc).to_owned(),
        systemprompt_ai::AiService::default_model(&svc).to_owned(),
        systemprompt_ai::AiService::default_max_output_tokens(&svc),
    );

    let dynamic: Arc<dyn AiProvider> = Arc::new(svc);
    assert_eq!(dynamic.default_provider(), inherent.0);
    assert_eq!(dynamic.default_model(), inherent.1);
    assert_eq!(dynamic.default_max_output_tokens(), inherent.2);
    assert_eq!(
        dynamic.default_provider(),
        ANTHROPIC,
        "the configured default must survive the trait indirection"
    );
}

#[tokio::test]
async fn generate_through_the_trait_returns_the_upstream_content() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("via trait"))
            .await;
    let svc: Arc<dyn AiProvider> = Arc::new(service(&pool, ANTHROPIC, server.uri()));
    let (_user, context) = seeded_context(&pool).await;

    let response = svc.generate(&request(context)).await.expect("generate");
    assert!(response.content.contains("via trait"));
}

#[tokio::test]
async fn an_upstream_failure_surfaces_as_a_boxed_provider_error() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_error(500, serde_json::json!({"error":{"message":"boom"}}))
            .await;
    let svc: Arc<dyn AiProvider> = Arc::new(service(&pool, ANTHROPIC, server.uri()));
    let (_user, context) = seeded_context(&pool).await;

    let err = svc
        .generate(&request(context))
        .await
        .expect_err("an upstream 500 must not be swallowed");
    assert!(
        !err.to_string().is_empty(),
        "the boxed error must carry the domain error's message"
    );
}

#[tokio::test]
async fn generate_with_tools_and_single_turn_go_through_the_trait() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("tooled")).await;
    let svc: Arc<dyn AiProvider> = Arc::new(service(&pool, ANTHROPIC, server.uri()));
    let (_user, context) = seeded_context(&pool).await;

    let tooled = svc
        .generate_with_tools(&request(context.clone()))
        .await
        .expect("generate_with_tools");
    assert!(tooled.content.contains("tooled"));

    let (single, calls) = svc
        .generate_single_turn(&request(context))
        .await
        .expect("generate_single_turn");
    assert!(single.content.contains("tooled"));
    assert!(
        calls.is_empty(),
        "a text-only upstream reply yields no tool calls, got {calls:?}"
    );
}

#[tokio::test]
async fn both_streaming_entry_points_yield_chunks_through_the_trait() {
    let Some(pool) = pool().await else {
        return;
    };
    let server = mock_http::anthropic_messages_stream(SSE).await;
    let svc: Arc<dyn AiProvider> = Arc::new(service(&pool, ANTHROPIC, server.uri()));
    let (_user, context) = seeded_context(&pool).await;

    let mut plain = svc
        .generate_stream(&request(context.clone()))
        .await
        .expect("generate_stream");
    let mut text = String::new();
    while let Some(chunk) = plain.next().await {
        if let StreamChunk::Text(t) = chunk.expect("stream item") {
            text.push_str(&t);
        }
    }
    assert!(text.contains("hi"), "the plain stream must carry the delta");

    let mut tooled = svc
        .generate_with_tools_stream(&request(context))
        .await
        .expect("generate_with_tools_stream");
    let mut tooled_text = String::new();
    while let Some(chunk) = tooled.next().await {
        if let StreamChunk::Text(t) = chunk.expect("stream item") {
            tooled_text.push_str(&t);
        }
    }
    assert!(tooled_text.contains("hi"));
}

#[tokio::test]
async fn tool_discovery_and_execution_go_through_the_trait() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("x")).await;
    let svc: Arc<dyn AiProvider> = Arc::new(service(&pool, ANTHROPIC, server.uri()));
    let (_user, context) = seeded_context(&pool).await;

    let tools = svc
        .list_available_tools_for_agent(&AgentName::new("ai-core-test"), &context)
        .await
        .expect("tool listing");
    assert!(
        tools.is_empty(),
        "the noop tool provider advertises nothing, got {tools:?}"
    );

    let declared = vec![McpTool::new("noop_tool", McpServerId::new("svc"))];
    let (calls, results) = svc.execute_tools(vec![], &declared, &context, None).await;
    assert!(
        calls.is_empty() && results.is_empty(),
        "no calls, no results"
    );
}

#[tokio::test]
async fn health_check_reports_a_status_per_configured_provider() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("x")).await;
    let svc: Arc<dyn AiProvider> = Arc::new(service(&pool, ANTHROPIC, server.uri()));

    let health = svc.health_check().await.expect("health check");
    assert_eq!(
        health.get("provider_anthropic"),
        Some(&true),
        "the only configured provider must report healthy against a live mock, got {health:?}"
    );
}

#[tokio::test]
async fn plan_and_response_generation_go_through_the_trait() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("planned")).await;
    let svc: Arc<dyn AiProvider> = Arc::new(service(&pool, ANTHROPIC, server.uri()));
    let (_user, context) = seeded_context(&pool).await;

    // With no tools declared the planner cannot schedule any step, so the plan
    // comes back empty rather than failing.
    let plan = svc
        .generate_plan(&request(context.clone()), &[])
        .await
        .expect("planning must be reachable through the trait");
    match plan {
        systemprompt_models::ai::PlanningResult::DirectResponse { content } => {
            assert!(
                content.contains("planned"),
                "with no tools declared the planner must answer directly from the model reply, \
                 got {content}"
            );
        },
        other => panic!("no tools were declared, so no tool calls can be planned: {other:?}"),
    }

    let text = svc
        .generate_response(GenerateResponseParams {
            messages: vec![AiMessage::user("original question")],
            execution_summary: "tool A returned 42",
            context: &context,
            provider: Some(ANTHROPIC),
            model: Some(MODEL),
            max_output_tokens: Some(64),
        })
        .await
        .expect("generate_response");
    assert!(text.contains("planned"));
}

#[tokio::test]
async fn google_search_through_the_trait_is_rejected_for_a_non_gemini_default() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("x")).await;
    let svc: Arc<dyn AiProvider> = Arc::new(service(&pool, ANTHROPIC, server.uri()));
    let (_user, _context) = seeded_context(&pool).await;

    let result = svc
        .generate_with_google_search(GoogleSearchParams {
            messages: vec![AiMessage::user("who won")],
            sampling: None,
            max_output_tokens: 64,
            model: None,
            urls: None,
            response_schema: None,
        })
        .await;
    assert!(
        result.is_err(),
        "search grounding is a Gemini capability; an Anthropic-only service must refuse it"
    );
}
