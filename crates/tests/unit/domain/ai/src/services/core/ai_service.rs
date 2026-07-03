// AiService pipeline tests against wiremock provider endpoints.

use futures::StreamExt;
use serde_json::json;
use systemprompt_ai::models::ai::{AiMessage, AiRequest, GenerateResponseParams, StreamChunk};
use systemprompt_ai::models::tools::McpTool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{McpServerId, UserId};
use systemprompt_models::ai::PlanningResult;

use super::{pool, seeded_context, service};
use crate::services::providers::mock_http;

const ANTHROPIC: &str = "anthropic";
const ANTHROPIC_MODEL: &str = "claude-sonnet-4-6";
const OPENAI: &str = "openai";
const OPENAI_MODEL: &str = "gpt-4.1";

// Valid anthropic SSE stream body (mirrors the provider-level streaming test).
const ANTHROPIC_SSE: &str = "data: {\"type\":\"message_start\",\"message\":{\"id\":\"x\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":3,\"output_tokens\":1}}}\n\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}\n\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\n";

fn user_request(model: &str, context: systemprompt_models::RequestContext) -> AiRequest {
    AiRequest::builder(
        vec![AiMessage::system("be brief"), AiMessage::user("hi")],
        ANTHROPIC,
        model,
        128,
        context,
    )
    .build()
}

async fn count_requests(pool: &DbPool, user_id: &UserId) -> i64 {
    let read = pool.pool_arc().expect("read pool");
    sqlx::query_scalar!(
        "SELECT COUNT(*) FROM ai_requests WHERE user_id = $1",
        user_id.as_str()
    )
    .fetch_one(read.as_ref())
    .await
    .expect("count")
    .unwrap_or(0)
}

#[tokio::test]
async fn generate_returns_content_and_persists_audit_row() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("hello there"))
            .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user_id, ctx) = seeded_context(&pool).await;
    let request = user_request(ANTHROPIC_MODEL, ctx);

    let response = svc.generate(&request).await.expect("generate ok");
    assert!(response.content.contains("hello there"));
    assert_eq!(response.provider, ANTHROPIC);
    assert_eq!(response.input_tokens, Some(10));
    assert_eq!(response.output_tokens, Some(20));

    // audit() is awaited inline, so the row is durable on return.
    assert_eq!(count_requests(&pool, &user_id).await, 1);
}

#[tokio::test]
async fn generate_error_path_persists_failed_row_and_errs() {
    let Some(pool) = pool().await else {
        return;
    };
    let server = mock_http::anthropic_messages_error(
        400,
        json!({ "error": { "type": "invalid_request", "message": "bad" } }),
    )
    .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user_id, ctx) = seeded_context(&pool).await;
    let request = user_request(ANTHROPIC_MODEL, ctx);

    let err = svc.generate(&request).await.expect_err("must fail");
    assert!(!format!("{err:?}").is_empty());
    // The failed attempt is still audited.
    assert_eq!(count_requests(&pool, &user_id).await, 1);
}

#[tokio::test]
async fn generate_with_tools_single_text_turn() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("plain answer"))
            .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (_user, ctx) = seeded_context(&pool).await;
    let request = user_request(ANTHROPIC_MODEL, ctx);

    let response = svc.generate_with_tools(&request).await.expect("tools ok");
    assert!(response.content.contains("plain answer"));
}

#[tokio::test]
async fn generate_single_turn_returns_tool_calls() {
    let Some(pool) = pool().await else {
        return;
    };
    let server = mock_http::anthropic_messages_success(mock_http::anthropic_tool_use_body(
        "lookup",
        json!({ "q": "x" }),
    ))
    .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (_user, ctx) = seeded_context(&pool).await;
    let request = user_request(ANTHROPIC_MODEL, ctx);

    let (response, calls) = svc
        .generate_single_turn(&request)
        .await
        .expect("single turn ok");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "lookup");
    assert!(response.content.contains("calling tool"));
}

#[tokio::test]
async fn generate_plan_direct_response_when_no_tool_calls() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("just reasoning"))
            .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (_user, ctx) = seeded_context(&pool).await;
    let request = user_request(ANTHROPIC_MODEL, ctx);

    let plan = svc.generate_plan(&request, &[]).await.expect("plan ok");
    match plan {
        PlanningResult::DirectResponse { content } => assert!(content.contains("just reasoning")),
        other => panic!("expected DirectResponse, got {other:?}"),
    }
}

#[tokio::test]
async fn generate_plan_tool_calls_when_present() {
    let Some(pool) = pool().await else {
        return;
    };
    let server = mock_http::anthropic_messages_success(mock_http::anthropic_tool_use_body(
        "search",
        json!({ "query": "rust" }),
    ))
    .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (_user, ctx) = seeded_context(&pool).await;
    let request = user_request(ANTHROPIC_MODEL, ctx);
    let tools = vec![McpTool::new("search", McpServerId::new("svc"))];

    let plan = svc.generate_plan(&request, &tools).await.expect("plan ok");
    match plan {
        PlanningResult::ToolCalls { calls, .. } => {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].tool_name, "search");
        },
        other => panic!("expected ToolCalls, got {other:?}"),
    }
}

#[tokio::test]
async fn generate_response_synthesizes_final_text() {
    let Some(pool) = pool().await else {
        return;
    };
    let server = mock_http::anthropic_messages_success(mock_http::anthropic_response_body(
        "final synthesized answer",
    ))
    .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (_user, ctx) = seeded_context(&pool).await;

    let params = GenerateResponseParams {
        messages: vec![AiMessage::user("original question")],
        execution_summary: "tool A returned 42",
        context: &ctx,
        provider: Some(ANTHROPIC),
        model: Some(ANTHROPIC_MODEL),
        max_output_tokens: Some(64),
    };
    let text = svc.generate_response(params).await.expect("response ok");
    assert!(text.contains("final synthesized answer"));
}

#[tokio::test]
async fn generate_response_falls_back_to_defaults_when_unset() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("defaulted"))
            .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (_user, ctx) = seeded_context(&pool).await;

    // provider/model unset → service uses default_provider/default_model.
    let params = GenerateResponseParams {
        messages: vec![AiMessage::user("q")],
        execution_summary: "summary",
        context: &ctx,
        provider: None,
        model: None,
        max_output_tokens: None,
    };
    let text = svc.generate_response(params).await.expect("ok");
    assert!(text.contains("defaulted"));
}

#[tokio::test]
async fn generate_stream_yields_text_chunks() {
    let Some(pool) = pool().await else {
        return;
    };
    let server = mock_http::anthropic_messages_stream(ANTHROPIC_SSE).await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (_user, ctx) = seeded_context(&pool).await;
    let request = user_request(ANTHROPIC_MODEL, ctx);

    let mut stream = svc.generate_stream(&request).await.expect("stream ok");
    let mut text = String::new();
    let mut count = 0_usize;
    while let Some(chunk) = stream.next().await {
        if let StreamChunk::Text(t) = chunk.expect("chunk ok") {
            text.push_str(&t);
        }
        count += 1;
        if count > 20 {
            break;
        }
    }
    assert!(text.contains("hello"));
}

#[tokio::test]
async fn generate_with_tools_stream_yields_chunks() {
    let Some(pool) = pool().await else {
        return;
    };
    let server = mock_http::anthropic_messages_stream(ANTHROPIC_SSE).await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (_user, ctx) = seeded_context(&pool).await;
    let request = user_request(ANTHROPIC_MODEL, ctx);

    let mut stream = svc
        .generate_with_tools_stream(&request)
        .await
        .expect("tool stream ok");
    let mut count = 0_usize;
    let mut text = String::new();
    while let Some(chunk) = stream.next().await {
        if let StreamChunk::Text(t) = chunk.expect("chunk ok") {
            text.push_str(&t);
        }
        count += 1;
        if count > 20 {
            break;
        }
    }
    assert!(count >= 1);
    assert!(
        text.contains("hello"),
        "streamed text missing delta content: {text}"
    );
}

#[tokio::test]
async fn health_check_reports_provider_and_tools() {
    let Some(pool) = pool().await else {
        return;
    };
    let server = mock_http::anthropic_messages_success(json!({})).await;
    let svc = service(&pool, ANTHROPIC, server.uri());

    let health = svc.health_check().await.expect("health ok");
    assert_eq!(health.get("provider_anthropic"), Some(&true));
}

#[tokio::test]
async fn default_getters_reflect_config() {
    let Some(pool) = pool().await else {
        return;
    };
    let server = mock_http::anthropic_messages_success(json!({})).await;
    let svc = service(&pool, ANTHROPIC, server.uri());

    assert_eq!(svc.default_provider(), ANTHROPIC);
    assert_eq!(svc.default_model(), ANTHROPIC_MODEL);
    assert_eq!(svc.default_max_output_tokens(), 512);
}

#[tokio::test]
async fn unknown_provider_in_request_errors() {
    let Some(pool) = pool().await else {
        return;
    };
    let server = mock_http::anthropic_messages_success(json!({})).await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (_user, ctx) = seeded_context(&pool).await;
    let request = AiRequest::builder(
        vec![AiMessage::user("hi")],
        "nonexistent-provider",
        ANTHROPIC_MODEL,
        64,
        ctx,
    )
    .build();

    let err = svc.generate(&request).await.expect_err("must fail");
    assert!(format!("{err:?}").contains("nonexistent-provider"));
}

#[tokio::test]
async fn openai_protocol_drives_generate() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::openai_chat_success(mock_http::openai_response_body("openai answer")).await;
    let svc = service(&pool, OPENAI, server.uri());
    let (_user, ctx) = seeded_context(&pool).await;
    let request =
        AiRequest::builder(vec![AiMessage::user("hi")], OPENAI, OPENAI_MODEL, 64, ctx).build();

    let response = svc.generate(&request).await.expect("openai generate ok");
    assert!(response.content.contains("openai answer"));
    assert_eq!(response.provider, OPENAI);
}

#[tokio::test]
async fn build_fails_when_default_provider_not_enabled() {
    let Some(pool) = pool().await else {
        return;
    };
    // default_provider points at a provider with no enabled policy entry.
    let registry = super::registry_with_endpoint(ANTHROPIC, "http://127.0.0.1:1".to_owned());
    let mut config = super::ai_config(ANTHROPIC);
    config.default_provider = "gemini".to_owned();
    let result = systemprompt_ai::AiService::new(
        &pool,
        &registry,
        &config,
        std::sync::Arc::new(systemprompt_ai::NoopToolProvider::new()),
        None,
    );
    assert!(result.is_err());
}
