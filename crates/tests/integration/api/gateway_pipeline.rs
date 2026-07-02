//! End-to-end coverage for the gateway dispatch pipeline.
//! `GatewayService::dispatch` is driven directly against a wiremock upstream
//! provider so the full flow — route/provider resolution, secret + adapter
//! lookup, policy and quota checks, upstream send, and buffered/streaming
//! finalization with audit completion — runs against live Postgres. The gateway
//! config and provider registry are built in-test and point at the wiremock
//! endpoint; the provider api-key secret resolves from `ANTHROPIC_API_KEY`, set
//! before the process bootstrap.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::body::to_bytes;
use bytes::Bytes;
use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_api::services::gateway::protocol::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, InboundAdapter, Role,
};
use systemprompt_api::services::gateway::service::{DispatchError, GatewayService};
use systemprompt_api::services::gateway::{DispatchInputs, GatewayRequestContext};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    AiRequestId, ContextId, GatewayConversationId, ModelId, ProviderId, RouteId, SecretName,
    TraceId,
};
use systemprompt_models::profile::{
    ApiSurface, GatewayConfig, GatewayRoute, ProviderEntry, ProviderModel, ProviderRegistry,
    WireProtocol,
};
use systemprompt_test_fixtures::{AuthedFixture, seed_admin_credential};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;

const API_KEY_ENV: &str = "ANTHROPIC_API_KEY";
const API_KEY_SECRET: &str = "anthropic";
const PROVIDER: &str = "anthropic";
const MODEL: &str = "claude-test-model";

fn install_provider_api_key() {
    // SAFETY: set before the process's first `SecretsBootstrap::try_init` (driven
    // by `setup_ctx`); process-local under nextest's per-test process model.
    unsafe {
        std::env::set_var(API_KEY_ENV, "sk-test-anthropic-key");
    }
}

fn provider_registry(endpoint: &str, provider: &str) -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![ProviderEntry {
            name: ProviderId::new(provider),
            wire: WireProtocol::Anthropic,
            surface: ApiSurface::Anthropic,
            endpoint: endpoint.to_owned(),
            api_key_secret: SecretName::new(API_KEY_SECRET),
            extra_headers: HashMap::new(),
            models: vec![ProviderModel {
                id: ModelId::new(MODEL),
                aliases: Vec::new(),
                upstream_model: None,
                pricing: Default::default(),
                capabilities: Default::default(),
                limits: Default::default(),
            }],
        }],
    }
}

fn gateway_config(route_provider: &str) -> GatewayConfig {
    let mut route = GatewayRoute {
        id: RouteId::new(""),
        model_pattern: "claude-*".to_owned(),
        provider: ProviderId::new(route_provider),
        upstream_model: None,
        extra_headers: HashMap::new(),
        pricing: None,
        when: None,
    };
    route.ensure_id();
    GatewayConfig {
        enabled: true,
        routes: vec![route],
        ..GatewayConfig::default()
    }
}

fn canonical_request(model: &str, stream: bool) -> CanonicalRequest {
    CanonicalRequest {
        model: model.to_owned(),
        system: Some("be brief".to_owned()),
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::Text("hello gateway".to_owned())],
        }],
        max_tokens: 256,
        temperature: Some(0.5),
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        stream,
        thinking: None,
        metadata: None,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
    }
}

fn raw_body(request: &CanonicalRequest) -> Bytes {
    Bytes::from(
        serde_json::to_vec(&serde_json::json!({
            "model": request.model,
            "max_tokens": request.max_tokens,
            "messages": [{"role": "user", "content": "hello gateway"}],
        }))
        .expect("serialize raw body"),
    )
}

fn dispatch_ctx(cred: &AuthedFixture, model: &str, stream: bool) -> GatewayRequestContext {
    GatewayRequestContext {
        ai_request_id: AiRequestId::generate(),
        user_id: cred.user_id.clone(),
        session_id: Some(cred.session_id.clone()),
        context_id: ContextId::generate(),
        gateway_conversation_id: Some(
            GatewayConversationId::try_new(format!(
                "ctx_{}",
                &uuid::Uuid::new_v4().simple().to_string()[..16]
            ))
            .expect("valid conversation id"),
        ),
        trace_id: Some(TraceId::generate()),
        provider: PROVIDER.to_owned(),
        requested_model: Some(model.to_owned()),
        model: model.to_owned(),
        max_tokens: Some(256),
        is_streaming: stream,
        wire_protocol: "anthropic-messages".to_owned(),
    }
}

fn inbound() -> Arc<dyn InboundAdapter> {
    Arc::new(AnthropicMessagesInbound)
}

fn inputs(cred: &AuthedFixture, request: CanonicalRequest, stream: bool) -> DispatchInputs {
    let body = raw_body(&request);
    let ctx = dispatch_ctx(cred, &request.model, stream);
    DispatchInputs {
        request,
        raw_body: body,
        ctx,
        inbound: inbound(),
    }
}

fn buffered_response_json() -> serde_json::Value {
    serde_json::json!({
        "id": "msg_upstream_1",
        "type": "message",
        "role": "assistant",
        "model": MODEL,
        "content": [{"type": "text", "text": "hello from upstream"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 11, "output_tokens": 7}
    })
}

fn streaming_sse_body() -> String {
    [
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_s\",\"model\":\"claude-test-model\",\"usage\":{\"input_tokens\":9,\"output_tokens\":0}}}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"streamed hello\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":6}}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    ]
    .concat()
}

async fn poll_completion(pool: &DbPool, id: &AiRequestId) -> Option<i32> {
    let pg = pool.pool_arc().expect("read pool");
    for _ in 0..50 {
        let row: Option<(Option<i32>,)> =
            sqlx::query_as("SELECT tokens_used FROM ai_requests WHERE id = $1")
                .bind(id.as_str())
                .fetch_optional(pg.as_ref())
                .await
                .expect("query ai_requests");
        if let Some((Some(tokens),)) = row {
            return Some(tokens);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    None
}

#[tokio::test]
async fn buffered_dispatch_returns_rendered_response_and_completes_audit() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "gw-buffered@example.invalid").await?;

    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(buffered_response_json()))
        .mount(&upstream)
        .await;

    let config = gateway_config(PROVIDER);
    let registry = provider_registry(&upstream.uri(), PROVIDER);
    let request = canonical_request(MODEL, false);
    let di = inputs(&cred, request, false);
    let request_id = di.ctx.ai_request_id.clone();

    let resp = GatewayService::dispatch(&config, &registry, &pool, di)
        .await
        .expect("buffered dispatch succeeds");
    assert_eq!(resp.status(), http::StatusCode::OK);
    assert!(resp.headers().contains_key("x-systemprompt-request-id"));

    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    let body: serde_json::Value = serde_json::from_slice(&bytes)?;
    let rendered = body.to_string();
    assert!(rendered.contains("hello from upstream"), "body: {rendered}");

    let tokens = poll_completion(&pool, &request_id).await;
    assert_eq!(
        tokens,
        Some(18),
        "input+output tokens recorded on completion"
    );
    Ok(())
}

#[tokio::test]
async fn streaming_dispatch_taps_events_and_completes_audit() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "gw-stream@example.invalid").await?;

    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(streaming_sse_body(), "text/event-stream"),
        )
        .mount(&upstream)
        .await;

    let config = gateway_config(PROVIDER);
    let registry = provider_registry(&upstream.uri(), PROVIDER);
    let request = canonical_request(MODEL, true);
    let di = inputs(&cred, request, true);
    let request_id = di.ctx.ai_request_id.clone();

    let resp = GatewayService::dispatch(&config, &registry, &pool, di)
        .await
        .expect("streaming dispatch succeeds");
    assert_eq!(resp.status(), http::StatusCode::OK);
    let ctype = resp
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(ctype.contains("event-stream"), "content-type: {ctype}");

    let bytes = to_bytes(resp.into_body(), 4 * 1024 * 1024).await?;
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("streamed hello") || text.contains("text_delta"),
        "tapped stream body: {text}"
    );

    let tokens = poll_completion(&pool, &request_id).await;
    assert!(
        tokens.is_some(),
        "streaming completion must record a token count"
    );
    Ok(())
}

#[tokio::test]
async fn missing_session_binding_is_pre_audit_error() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "gw-nosession@example.invalid").await?;

    let config = gateway_config(PROVIDER);
    let registry = provider_registry("http://127.0.0.1:1", PROVIDER);
    let request = canonical_request(MODEL, false);
    let mut di = inputs(&cred, request, false);
    di.ctx.session_id = None;

    let err = GatewayService::dispatch(&config, &registry, &pool, di)
        .await
        .expect_err("missing session binding must fail pre-audit");
    assert!(matches!(err, DispatchError::PreAudit(_)), "got {err:?}");
    Ok(())
}

#[tokio::test]
async fn unexposed_model_is_policy_denied() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "gw-denied@example.invalid").await?;

    let config = gateway_config(PROVIDER);
    let registry = provider_registry("http://127.0.0.1:1", PROVIDER);
    let request = canonical_request("ghost-model-not-exposed", false);
    let di = inputs(&cred, request, false);

    let err = GatewayService::dispatch(&config, &registry, &pool, di)
        .await
        .expect_err("unexposed model must be denied");
    match err {
        DispatchError::PreAudit(inner) => assert!(
            inner
                .downcast_ref::<systemprompt_api::services::gateway::service::PolicyDenied>()
                .is_some(),
            "expected PolicyDenied, got {inner}"
        ),
        other => panic!("expected PreAudit(PolicyDenied), got {other:?}"),
    }
    Ok(())
}

#[tokio::test]
async fn route_provider_absent_from_registry_is_pre_audit_error() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "gw-noprovider@example.invalid").await?;

    let config = gateway_config("ghost-provider");
    let registry = provider_registry("http://127.0.0.1:1", PROVIDER);
    let request = canonical_request(MODEL, false);
    let di = inputs(&cred, request, false);

    let err = GatewayService::dispatch(&config, &registry, &pool, di)
        .await
        .expect_err("route pointing at an unknown provider must fail");
    assert!(matches!(err, DispatchError::PreAudit(_)), "got {err:?}");
    Ok(())
}

#[tokio::test]
async fn missing_api_key_secret_is_pre_audit_error() -> anyhow::Result<()> {
    // Intentionally do NOT install the api key; the secret lookup must fail.
    unsafe {
        std::env::remove_var(API_KEY_ENV);
    }
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "gw-nokey@example.invalid").await?;

    let config = gateway_config(PROVIDER);
    let mut registry = provider_registry("http://127.0.0.1:1", PROVIDER);
    registry.providers[0].api_key_secret = SecretName::new("definitely_absent_secret_key");
    let request = canonical_request(MODEL, false);
    let di = inputs(&cred, request, false);

    let err = GatewayService::dispatch(&config, &registry, &pool, di)
        .await
        .expect_err("absent api key secret must fail pre-audit");
    assert!(matches!(err, DispatchError::PreAudit(_)), "got {err:?}");
    Ok(())
}

#[tokio::test]
async fn upstream_4xx_is_recorded_upstream_error() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "gw-upstream4xx@example.invalid").await?;

    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "type": "error",
            "error": {"type": "invalid_request_error", "message": "bad model"}
        })))
        .mount(&upstream)
        .await;

    let config = gateway_config(PROVIDER);
    let registry = provider_registry(&upstream.uri(), PROVIDER);
    let request = canonical_request(MODEL, false);
    let di = inputs(&cred, request, false);

    let err = GatewayService::dispatch(&config, &registry, &pool, di)
        .await
        .expect_err("upstream 400 must surface as a dispatch error");
    match err {
        DispatchError::Recorded(inner) => {
            let upstream_err = inner
                .downcast_ref::<systemprompt_api::services::gateway::protocol::outbound::UpstreamError>();
            assert!(
                upstream_err.is_some(),
                "expected UpstreamError, got {inner}"
            );
        },
        other => panic!("expected Recorded(UpstreamError), got {other:?}"),
    }
    Ok(())
}

#[tokio::test]
async fn upstream_5xx_is_recorded_upstream_error() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "gw-upstream5xx@example.invalid").await?;

    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(503).set_body_string("upstream unavailable"))
        .mount(&upstream)
        .await;

    let config = gateway_config(PROVIDER);
    let registry = provider_registry(&upstream.uri(), PROVIDER);
    let request = canonical_request(MODEL, false);
    let di = inputs(&cred, request, false);

    let err = GatewayService::dispatch(&config, &registry, &pool, di)
        .await
        .expect_err("upstream 503 must surface as a dispatch error");
    assert!(matches!(err, DispatchError::Recorded(_)), "got {err:?}");
    Ok(())
}
