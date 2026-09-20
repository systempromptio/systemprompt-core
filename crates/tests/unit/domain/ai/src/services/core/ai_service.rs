// AiService pipeline tests against wiremock provider endpoints.

use std::sync::atomic::{AtomicUsize, Ordering};

use futures::StreamExt;
use serde_json::json;
use systemprompt_ai::models::ai::{AiMessage, AiRequest, GenerateResponseParams, StreamChunk};
use systemprompt_ai::models::tools::McpTool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{McpServerId, UserId};
use systemprompt_models::ai::PlanningResult;

use super::{pool_or_skip, seeded_context, service};
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
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
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
    let tools = vec![McpTool::new(
        "search",
        McpServerId::try_new("svc").expect("valid McpServerId"),
    )];

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
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
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

struct StreamAudit {
    status: String,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    is_streaming: bool,
    cost_microdollars: i64,
    content_len: i32,
    reasoning_tokens: Option<i32>,
}

// The stream wrapper persists via tokio::spawn after the stream ends, so the
// row lands asynchronously; poll with a bounded deadline instead of sleeping a
// fixed interval.
async fn wait_for_streamed_row(pool: &DbPool, user_id: &UserId) -> StreamAudit {
    let read = pool.pool_arc().expect("read pool");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let row = sqlx::query!(
            r#"SELECT status as "status!", input_tokens, output_tokens, reasoning_tokens,
               is_streaming as "is_streaming!", cost_microdollars as "cost_microdollars!",
               LENGTH(COALESCE(m.content, '')) as "content_len!"
               FROM ai_requests r
               LEFT JOIN ai_request_messages m
                 ON m.request_id = r.id AND m.role = 'assistant'
               WHERE r.user_id = $1"#,
            user_id.as_str()
        )
        .fetch_optional(read.as_ref())
        .await
        .expect("query");
        // Messages are written after the ai_requests row; wait for the
        // assistant message so content_len is stable.
        if let Some(row) = row.filter(|r| r.content_len > 0) {
            return StreamAudit {
                status: row.status,
                input_tokens: row.input_tokens,
                output_tokens: row.output_tokens,
                is_streaming: row.is_streaming,
                cost_microdollars: row.cost_microdollars,
                content_len: row.content_len,
                reasoning_tokens: row.reasoning_tokens,
            };
        }
        assert!(
            std::time::Instant::now() < deadline,
            "streamed audit row never appeared for {user_id}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
}

// Both message_delta frames carry usage and both reach the wrapper as Usage
// chunks — including the one that also carries stop_reason, which is where
// Anthropic reports the real final counts. Usage is cumulative, so the last
// frame wins per field: output_tokens 5 from the final frame, input_tokens 3
// carried over from the frame that reported it.
const ANTHROPIC_SSE_WITH_USAGE: &str = "data: {\"type\":\"message_start\",\"message\":{\"id\":\"x\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":3,\"output_tokens\":1}}}\n\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}\n\ndata: {\"type\":\"message_delta\",\"usage\":{\"input_tokens\":3,\"output_tokens\":5}}\n\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\n";

#[tokio::test]
async fn drained_stream_persists_completed_audit_with_aggregated_usage() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let server = mock_http::anthropic_messages_stream(ANTHROPIC_SSE_WITH_USAGE).await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user_id, ctx) = seeded_context(&pool).await;
    let request = user_request(ANTHROPIC_MODEL, ctx);

    let mut stream = svc.generate_stream(&request).await.expect("stream ok");
    let mut text = String::new();
    while let Some(chunk) = stream.next().await {
        if let StreamChunk::Text(t) = chunk.expect("chunk ok") {
            text.push_str(&t);
        }
    }
    drop(stream);
    assert_eq!(text, "hello");

    let audit = wait_for_streamed_row(&pool, &user_id).await;
    assert_eq!(audit.status, "completed");
    assert!(audit.is_streaming);
    assert_eq!(audit.input_tokens, Some(3));
    assert_eq!(audit.output_tokens, Some(5));
    // Anthropic reports no separate thinking count; it is recorded as 0, the
    // same value the gateway writes, so both halves of the product agree.
    assert_eq!(audit.reasoning_tokens, Some(0));
    assert!(
        audit.cost_microdollars > 0,
        "priced model must accrue cost, got {}",
        audit.cost_microdollars
    );
    assert_eq!(
        audit.content_len,
        i32::try_from("hello".len()).expect("len")
    );
}

// Why: a consumer that drops the stream after the first chunk has still made
// the provider call; the audit row must record the abandoned request (and
// bill whatever usage had been reported) rather than leave no trace of it.
#[tokio::test]
async fn dropped_stream_persists_a_failed_audit_row_with_the_usage_seen_so_far() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let server = mock_http::anthropic_messages_stream(ANTHROPIC_SSE_WITH_USAGE).await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user_id, ctx) = seeded_context(&pool).await;
    let request = user_request(ANTHROPIC_MODEL, ctx);

    let mut stream = svc.generate_stream(&request).await.expect("stream ok");
    let first = stream
        .next()
        .await
        .expect("a first chunk")
        .expect("chunk ok");
    assert!(matches!(first, StreamChunk::Text(_)));
    drop(stream);

    let read = pool.pool_arc().expect("read pool");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let row = loop {
        let row = sqlx::query!(
            r#"SELECT status, is_streaming, input_tokens, cost_microdollars, error_message
               FROM ai_requests WHERE user_id = $1"#,
            user_id.as_str()
        )
        .fetch_optional(read.as_ref())
        .await
        .expect("query");
        if let Some(row) = row {
            break row;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "abandoned-stream audit row never appeared for {user_id}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    };
    assert_eq!(row.status, "failed");
    assert!(row.is_streaming);
    // Anthropic reports usage on the trailing message_delta frames, which the
    // consumer never reached: nothing was seen, so nothing is billed, and the
    // row still exists to say so.
    assert_eq!(row.input_tokens, None);
    assert_eq!(row.cost_microdollars, 0);
    assert!(
        row.error_message
            .as_deref()
            .is_some_and(|m| m.contains("dropped by the consumer")),
        "{:?}",
        row.error_message
    );
}

// Why: a request for a model the catalogue does not price cannot be settled;
// it is refused before the provider is called instead of billed at zero.
#[tokio::test]
async fn a_model_without_catalogue_pricing_is_refused_before_streaming() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let server = mock_http::anthropic_messages_stream(ANTHROPIC_SSE).await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (_user_id, ctx) = seeded_context(&pool).await;
    let request = user_request("claude-not-in-catalogue", ctx);

    let err = svc
        .generate_stream(&request)
        .await
        .err()
        .expect("an unpriced model is refused");
    assert!(err.to_string().contains("no pricing"), "{err}");
    assert!(
        server.received_requests().await.unwrap().is_empty(),
        "the provider is never called for an unpriced model"
    );
}

#[tokio::test]
async fn tool_stream_drained_to_end_persists_completed_audit() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let server = mock_http::anthropic_messages_stream(ANTHROPIC_SSE).await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user_id, ctx) = seeded_context(&pool).await;
    let request = user_request(ANTHROPIC_MODEL, ctx);

    let mut stream = svc
        .generate_with_tools_stream(&request)
        .await
        .expect("tool stream ok");
    while let Some(chunk) = stream.next().await {
        chunk.expect("chunk ok");
    }
    drop(stream);

    let audit = wait_for_streamed_row(&pool, &user_id).await;
    assert_eq!(audit.status, "completed");
    assert!(audit.is_streaming);
}

#[tokio::test]
async fn stream_connect_failure_surfaces_provider_error() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let server = mock_http::anthropic_messages_error(
        500,
        json!({ "error": { "type": "overloaded", "message": "busy" } }),
    )
    .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user_id, ctx) = seeded_context(&pool).await;
    let request = user_request(ANTHROPIC_MODEL, ctx);

    let result = svc.generate_stream(&request).await;
    let Err(err) = result else {
        panic!("stream connect against a 500 endpoint must fail");
    };
    assert!(!err.to_string().is_empty());
    assert_eq!(count_requests(&pool, &user_id).await, 0);
}

#[tokio::test]
async fn health_check_reports_provider_and_tools() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let server = mock_http::anthropic_messages_success(json!({})).await;
    let svc = service(&pool, ANTHROPIC, server.uri());

    let health = svc.health_check().await.expect("health ok");
    assert_eq!(health.get("provider_anthropic"), Some(&true));
}

#[tokio::test]
async fn default_getters_reflect_config() {
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
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
    let Some(pool) = pool_or_skip().await else {
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
        systemprompt_ai::AiServiceProviders {
            tools: std::sync::Arc::new(systemprompt_ai::NoopToolProvider::new()),
            sessions: super::noop_session_provider(),
        },
        &systemprompt_ai::repository::AiRepositories::new(&pool).expect("ai repositories"),
    );
    assert!(result.is_err());
}

#[tokio::test]
async fn google_search_errs_when_no_provider_supports_it() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let server = mock_http::anthropic_messages_success(json!({})).await;
    let svc = service(&pool, ANTHROPIC, server.uri());

    let err = svc
        .generate_with_google_search(systemprompt_ai::models::ai::GoogleSearchParams {
            messages: vec![AiMessage::user("what is new?")],
            sampling: None,
            max_output_tokens: 64,
            model: None,
            urls: None,
            response_schema: None,
        })
        .await
        .expect_err("anthropic-only service has no search-capable provider");
    assert!(format!("{err}").contains("Google Search"), "err: {err}");
}

#[tokio::test]
async fn google_search_uses_search_capable_provider_and_surfaces_sources() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let server =
        mock_http::gemini_generate_success(mock_http::gemini_grounded_body("grounded answer"))
            .await;
    let registry = super::registry_with_endpoint("gemini", server.uri());
    let mut config = super::ai_config("gemini");
    config
        .providers
        .get_mut("gemini")
        .expect("gemini policy entry")
        .google_search_enabled = true;
    let svc = systemprompt_ai::AiService::new(
        &pool,
        &registry,
        &config,
        systemprompt_ai::AiServiceProviders {
            tools: std::sync::Arc::new(systemprompt_ai::NoopToolProvider::new()),
            sessions: super::noop_session_provider(),
        },
        &systemprompt_ai::repository::AiRepositories::new(&pool).expect("ai repositories"),
    )
    .expect("AiService builds");

    let response = svc
        .generate_with_google_search(systemprompt_ai::models::ai::GoogleSearchParams {
            messages: vec![AiMessage::user("what is new?")],
            sampling: Some(systemprompt_ai::models::ai::SamplingParams::default()),
            max_output_tokens: 64,
            model: None,
            urls: Some(vec!["https://example.com/a".to_owned()]),
            response_schema: Some(json!({"type": "object"})),
        })
        .await
        .expect("grounded generation ok");

    assert!(response.content.contains("grounded answer"));
    assert_eq!(response.sources.len(), 1);
    assert_eq!(response.sources[0].uri, "https://example.com/a");
    assert_eq!(response.web_search_queries, vec!["test query".to_owned()]);
}

#[derive(Clone)]
struct ToolThenSynthesis {
    calls: std::sync::Arc<AtomicUsize>,
}

impl wiremock::Respond for ToolThenSynthesis {
    fn respond(&self, _request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let invocation = self.calls.fetch_add(1, Ordering::SeqCst);
        let body = if invocation == 0 {
            json!({
                "id": "msg_tool_only",
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": "toolu_synthesis",
                    "name": "lookup",
                    "input": {"query": "retained evidence"}
                }],
                "model": ANTHROPIC_MODEL,
                "stop_reason": "tool_use",
                "stop_sequence": null,
                "usage": {"input_tokens": 12, "output_tokens": 8}
            })
        } else {
            mock_http::anthropic_response_body("synthesized from tool outcome")
        };
        wiremock::ResponseTemplate::new(200).set_body_json(body)
    }
}

#[tokio::test]
async fn tools_only_response_synthesizes_and_audits_both_provider_calls() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer};

    let pool = pool_or_skip()
        .await
        .expect("AI database fixture must be configured");
    let server = MockServer::start().await;
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ToolThenSynthesis {
            calls: std::sync::Arc::clone(&calls),
        })
        .mount(&server)
        .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user, ctx) = seeded_context(&pool).await;
    let request = AiRequest::builder(
        vec![AiMessage::user("look up retained evidence")],
        ANTHROPIC,
        ANTHROPIC_MODEL,
        128,
        ctx,
    )
    .with_tools(vec![McpTool::new(
        "lookup",
        McpServerId::try_new("synthesis-fixture").expect("valid MCP server id"),
    )])
    .build();

    let response = svc
        .generate_with_tools(&request)
        .await
        .expect("tools-only provider turn synthesizes a final response");
    assert_eq!(response.content, "synthesized from tool outcome");
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_calls[0].name, "lookup");
    assert_eq!(response.tool_results.len(), 1);
    assert_eq!(response.tool_results[0].is_error, Some(true));
    let tool_diagnosis = serde_json::to_string(&response.tool_results[0].content)
        .expect("tool failure content serializes");
    assert!(
        tool_diagnosis.contains("NoopToolProvider cannot execute tool: lookup"),
        "synthesis must consume the real tool failure: {tool_diagnosis}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    let rows: Vec<(String, Option<i32>, Option<i32>)> = sqlx::query_as(
        "SELECT status, input_tokens, output_tokens FROM ai_requests WHERE user_id = $1 ORDER BY created_at, id",
    )
    .bind(user.as_str())
    .fetch_all(pool.pool_arc().expect("AI read pool").as_ref())
    .await
    .expect("durable primary and synthesis audit rows");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row.0 == "completed"));
    assert!(rows.contains(&("completed".to_owned(), Some(10), Some(20))));
    assert!(rows.contains(&("completed".to_owned(), Some(12), Some(8))));
}

#[derive(Clone)]
struct ToolThenProviderFailures {
    calls: std::sync::Arc<AtomicUsize>,
}

impl wiremock::Respond for ToolThenProviderFailures {
    fn respond(&self, _request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let invocation = self.calls.fetch_add(1, Ordering::SeqCst);
        if invocation == 0 {
            return wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "id": "msg_tool_before_failure",
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": "toolu_failure",
                    "name": "lookup",
                    "input": {"query": "failure evidence"}
                }],
                "model": ANTHROPIC_MODEL,
                "stop_reason": "tool_use",
                "stop_sequence": null,
                "usage": {"input_tokens": 12, "output_tokens": 8}
            }));
        }
        wiremock::ResponseTemplate::new(503)
            .set_body_json(json!({"error":{"message":"synthesis upstream unavailable"}}))
    }
}

#[tokio::test]
async fn failed_tool_synthesis_returns_diagnostic_fallback_without_fabricated_audit_calls() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer};

    let pool = pool_or_skip()
        .await
        .expect("AI database fixture must be configured");
    let server = MockServer::start().await;
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ToolThenProviderFailures {
            calls: std::sync::Arc::clone(&calls),
        })
        .mount(&server)
        .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user, ctx) = seeded_context(&pool).await;
    let request = AiRequest::builder(
        vec![AiMessage::user("look up failure evidence")],
        ANTHROPIC,
        ANTHROPIC_MODEL,
        128,
        ctx,
    )
    .with_tools(vec![McpTool::new(
        "lookup",
        McpServerId::try_new("synthesis-failure-fixture").expect("valid MCP server id"),
    )])
    .build();

    let response = svc
        .generate_with_tools(&request)
        .await
        .expect("failed synthesis returns an explicit tool-result fallback");
    assert!(response.content.contains("Tool execution completed"));
    assert!(response.content.contains("Synthesis error"));
    assert!(
        response
            .content
            .contains("Provider anthropic returned HTTP 503"),
        "fallback must carry the upstream diagnosis: {}",
        response.content
    );
    assert!(
        response.content.contains("synthesis upstream unavailable"),
        "fallback must retain the upstream response body: {}",
        response.content
    );
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_results.len(), 1);
    assert_eq!(response.tool_results[0].is_error, Some(true));
    let raw_tool_error = serde_json::to_string(&response.tool_results[0].content)
        .expect("tool failure content serializes");
    assert!(
        raw_tool_error.contains("NoopToolProvider cannot execute tool: lookup"),
        "raw tool result must retain the execution failure: {raw_tool_error}"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        7,
        "one initial tool turn plus three configured attempts for each failed synthesis stage"
    );

    let rows: Vec<(String, Option<i32>, Option<i32>)> = sqlx::query_as(
        "SELECT status, input_tokens, output_tokens FROM ai_requests WHERE user_id = $1",
    )
    .bind(user.as_str())
    .fetch_all(pool.pool_arc().expect("AI read pool").as_ref())
    .await
    .expect("durable audit rows after synthesis failure");
    assert_eq!(
        rows,
        vec![("completed".to_owned(), Some(12), Some(8))],
        "failed upstream synthesis attempts must not fabricate completed provider-call audits"
    );
}

#[derive(Clone)]
struct ToolThenEmptyThenGuidance {
    calls: std::sync::Arc<AtomicUsize>,
}

impl wiremock::Respond for ToolThenEmptyThenGuidance {
    fn respond(&self, _request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let invocation = self.calls.fetch_add(1, Ordering::SeqCst);
        let body = match invocation {
            0 => json!({
                "id": "msg_tool_before_guidance",
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": "toolu_guidance",
                    "name": "lookup",
                    "input": {"query": "guidance evidence"}
                }],
                "model": ANTHROPIC_MODEL,
                "stop_reason": "tool_use",
                "stop_sequence": null,
                "usage": {"input_tokens": 12, "output_tokens": 8}
            }),
            1 => mock_http::anthropic_response_body(""),
            _ => mock_http::anthropic_response_body("recovered through guidance"),
        };
        wiremock::ResponseTemplate::new(200).set_body_json(body)
    }
}

#[tokio::test]
async fn empty_tool_synthesis_retries_guidance_and_audits_every_completed_provider_call() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer};

    let pool = pool_or_skip()
        .await
        .expect("AI database fixture must be configured");
    let server = MockServer::start().await;
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ToolThenEmptyThenGuidance {
            calls: std::sync::Arc::clone(&calls),
        })
        .mount(&server)
        .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user, ctx) = seeded_context(&pool).await;
    let request = AiRequest::builder(
        vec![AiMessage::user("look up guidance evidence")],
        ANTHROPIC,
        ANTHROPIC_MODEL,
        128,
        ctx,
    )
    .with_tools(vec![McpTool::new(
        "lookup",
        McpServerId::try_new("guidance-fixture").expect("valid MCP server id"),
    )])
    .build();

    let response = svc
        .generate_with_tools(&request)
        .await
        .expect("empty first synthesis retries with guidance");
    assert_eq!(response.content, "recovered through guidance");
    assert_eq!(calls.load(Ordering::SeqCst), 3);

    let rows: Vec<(String, Option<i32>, Option<i32>)> = sqlx::query_as(
        "SELECT status, input_tokens, output_tokens FROM ai_requests WHERE user_id = $1",
    )
    .bind(user.as_str())
    .fetch_all(pool.pool_arc().expect("AI read pool").as_ref())
    .await
    .expect("durable audit rows after guidance retry");
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|row| row.0 == "completed"));
    assert_eq!(
        rows.iter()
            .filter(|row| row.1 == Some(10) && row.2 == Some(20))
            .count(),
        2,
        "empty synthesis and successful guidance are both billable completed calls"
    );
    assert!(rows.contains(&("completed".to_owned(), Some(12), Some(8))));
}
