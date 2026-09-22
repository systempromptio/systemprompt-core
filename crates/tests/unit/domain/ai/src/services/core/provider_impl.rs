// The `AiProvider` trait impl on `AiService` — the seam every consumer that
// holds an `Arc<dyn AiProvider>` goes through. It is a separate surface from
// the inherent methods `ai_service.rs` exercises: each trait method re-enters
// the inherent one and maps the domain error into a boxed provider error, and
// nothing else in the suite calls it.

use std::sync::Arc;

use futures::StreamExt;
use systemprompt_ai::models::ai::{AiMessage, AiRequest};
use systemprompt_ai::models::tools::McpTool;
use systemprompt_ai::{AiService, NoopToolProvider};
use systemprompt_identifiers::{AgentName, McpServerId};
use systemprompt_models::ai::{
    AiProvider, GenerateResponseParams, GoogleSearchParams, StreamChunk,
};
use systemprompt_models::errors::AiInferenceError;

use super::{
    ai_config, noop_session_provider, pool_or_skip, registry_with_endpoint, seeded_context, service,
};
use crate::services::providers::mock_http;

const ANTHROPIC: &str = "anthropic";
const MODEL: &str = "claude-sonnet-5";

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
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
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
    let pool = pool_or_skip()
        .await
        .expect("AI provider trait database fixture");
    let server =
        mock_http::anthropic_messages_error(500, serde_json::json!({"error":{"message":"boom"}}))
            .await;
    let registry = registry_with_endpoint(ANTHROPIC, server.uri());
    let mut config = ai_config(ANTHROPIC);
    config
        .providers
        .get_mut(ANTHROPIC)
        .expect("configured anthropic policy")
        .resilience
        .retry_attempts = 1;
    let service = Arc::new(
        AiService::new(
            &pool,
            &registry,
            &config,
            systemprompt_ai::AiServiceProviders {
                tools: Arc::new(NoopToolProvider::new()),
                sessions: noop_session_provider(),
            },
            &systemprompt_ai::repository::AiRepositories::new(&pool).expect("AI repositories"),
        )
        .expect("service with one-attempt provider builds"),
    );
    let svc: Arc<dyn AiProvider> = service.clone();
    let (user, context) = seeded_context(&pool).await;

    let err = svc
        .generate(&request(context))
        .await
        .expect_err("an upstream 500 must not be swallowed");
    assert!(matches!(
        err,
        AiInferenceError::Unavailable { provider, message }
            if provider == ANTHROPIC && message.contains("HTTP 500") && message.contains("boom")
    ));
    assert_eq!(
        server
            .received_requests()
            .await
            .expect("recorded requests")
            .len(),
        1,
        "the service provider seam must surface this failed upstream call instead of dispatching a second request"
    );
    service.audit_tasks().close();
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        service.audit_tasks().wait(),
    )
    .await
    .expect("failed audit tasks finish within 10 seconds");
    let failed: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM ai_requests WHERE user_id = $1 AND status = 'failed'",
        user.as_str()
    )
    .fetch_one(pool.pool_arc().expect("read pool").as_ref())
    .await
    .expect("failed audit count")
    .unwrap_or(0);
    assert_eq!(
        failed, 1,
        "the provider-facing error must still leave exactly one failed audit row"
    );
}

#[tokio::test]
async fn trait_rejects_an_unpriced_explicit_model_before_dispatch_or_audit() {
    let pool = pool_or_skip()
        .await
        .expect("AI provider trait database fixture");
    let server = mock_http::anthropic_messages_stream(SSE).await;
    let service = Arc::new(service(&pool, ANTHROPIC, server.uri()));
    let provider: Arc<dyn AiProvider> = service.clone();
    let (user, context) = seeded_context(&pool).await;
    let request = AiRequest::builder(
        vec![AiMessage::user("hi")],
        ANTHROPIC,
        "not-priced-by-the-catalogue",
        128,
        context,
    )
    .build();

    let Err(err) = provider.generate_stream(&request).await else {
        panic!("an unpriced model must be rejected before opening a stream");
    };
    assert!(matches!(
        err,
        AiInferenceError::Configuration(message)
            if message.contains("anthropic") && message.contains("not-priced-by-the-catalogue")
    ));
    assert!(
        server
            .received_requests()
            .await
            .expect("recorded requests")
            .is_empty(),
        "pricing validation must reject the request before the upstream stream is opened"
    );
    service.audit_tasks().close();
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        service.audit_tasks().wait(),
    )
    .await
    .expect("rejected stream audit tasks finish within 10 seconds");
    let rows: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM ai_requests WHERE user_id = $1",
        user.as_str()
    )
    .fetch_one(pool.pool_arc().expect("read pool").as_ref())
    .await
    .expect("audit row count")
    .unwrap_or(0);
    assert_eq!(
        rows, 0,
        "a request rejected before dispatch has no audit row"
    );
}

#[tokio::test]
async fn trait_rejects_an_unconfigured_provider_without_dispatch_or_audit() {
    let pool = pool_or_skip()
        .await
        .expect("AI provider trait database fixture");
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("unused")).await;
    let service = Arc::new(service(&pool, ANTHROPIC, server.uri()));
    let provider: Arc<dyn AiProvider> = service.clone();
    let (user, context) = seeded_context(&pool).await;
    let request = AiRequest::builder(
        vec![AiMessage::user("hi")],
        "not-configured",
        MODEL,
        128,
        context,
    )
    .build();

    let err = provider
        .generate(&request)
        .await
        .expect_err("an unconfigured provider cannot be dispatched");
    assert!(matches!(
        err,
        AiInferenceError::Internal(message)
            if message.contains("Provider not-configured not found")
    ));
    assert!(
        server
            .received_requests()
            .await
            .expect("recorded requests")
            .is_empty(),
        "provider lookup must fail locally without sending the request to another configured provider"
    );
    service.audit_tasks().close();
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        service.audit_tasks().wait(),
    )
    .await
    .expect("rejected provider audit tasks finish within 10 seconds");
    let rows: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM ai_requests WHERE user_id = $1",
        user.as_str()
    )
    .fetch_one(pool.pool_arc().expect("read pool").as_ref())
    .await
    .expect("audit row count")
    .unwrap_or(0);
    assert_eq!(rows, 0, "a provider lookup rejection has no audit row");
}

#[tokio::test]
async fn generate_with_tools_and_single_turn_go_through_the_trait() {
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("x")).await;
    let svc: Arc<dyn AiProvider> = Arc::new(service(&pool, ANTHROPIC, server.uri()));
    let (_user, context) = seeded_context(&pool).await;

    let tools = svc
        .list_available_tools_for_agent(
            &AgentName::try_new("ai-core-test").expect("valid AgentName"),
            &context,
        )
        .await
        .expect("tool listing");
    assert!(
        tools.is_empty(),
        "the noop tool provider advertises nothing, got {tools:?}"
    );

    let declared = vec![McpTool::new(
        "noop_tool",
        McpServerId::try_new("svc").expect("valid McpServerId"),
    )];
    let (calls, results) = svc.execute_tools(vec![], &declared, &context, None).await;
    assert!(
        calls.is_empty() && results.is_empty(),
        "no calls, no results"
    );
}

#[tokio::test]
async fn health_check_reports_a_status_per_configured_provider() {
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
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
