//! An upstream stream that ends with no terminal event, on every inbound
//! surface.
//!
//! The audit already recorded these turns as failed, but the caller saw only a
//! closed socket: no `message_stop` on the Anthropic surface, no `[DONE]` on
//! Chat Completions, no `response.completed` on Responses. A client cannot
//! tell that apart from a hung connection, so it waits for the full request
//! timeout on a turn the gateway finished seconds earlier. Each cell here
//! drives a real dispatch against a wiremock upstream whose SSE body simply
//! stops, and asserts the surface states the failure in its own vocabulary.

use std::sync::Arc;

use axum::body::to_bytes;
use bytes::Bytes;
use systemprompt_api::services::gateway::protocol::InboundAdapter;
use systemprompt_api::services::gateway::service::GatewayService;
use systemprompt_models::services::{ApiSurface, WireProtocol};
use systemprompt_test_fixtures::seed_admin_credential;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;
use super::gateway_matrix_inbound::InWire;
use super::gateway_pipeline::{
    PROVIDER, gateway_config, gw_repos, inputs_with, install_provider_api_key, provider_registry,
};

// Why: an Anthropic-wire upstream that opens a message and a text block and
// then stops -- the shape observed live from Vertex `MaaS` and from Gemini
// streams that never send a finishReason.
fn headless_anthropic_sse() -> String {
    [
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_m\",\"model\":\"claude-test-model\",\"usage\":{\"input_tokens\":11,\"output_tokens\":0}}}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"partial\"}}\n\n",
    ]
    .concat()
}

async fn run_headless(label: &str, inbound: InWire) -> anyhow::Result<String> {
    install_provider_api_key();
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, &format!("gw-abort-{label}@example.invalid")).await?;

    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(headless_anthropic_sse(), "text/event-stream"),
        )
        .mount(&upstream)
        .await;

    let adapter: Arc<dyn InboundAdapter> = inbound.adapter();
    let raw: Bytes = inbound.request_body(true);
    let request = adapter
        .parse_request(&raw)
        .map_err(|e| anyhow::anyhow!("inbound parse failed: {e}"))?;
    let config = gateway_config(PROVIDER);
    let registry = provider_registry(
        &upstream.uri(),
        PROVIDER,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let di = inputs_with(&cred, request, true, Arc::clone(&adapter), raw);

    let resp = GatewayService::dispatch(&config, &registry, &pool, &gw_repos(&pool), di)
        .await
        .map_err(|e| anyhow::anyhow!("dispatch failed: {e:?}"))?;
    let bytes = to_bytes(resp.into_body(), 4 * 1024 * 1024).await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn anthropic_in_headless_stream_emits_error_event() -> anyhow::Result<()> {
    let rendered = run_headless("anthropic", InWire::Anthropic).await?;
    assert!(
        rendered.contains("event: error") && rendered.contains("\"type\":\"error\""),
        "the Anthropic surface must state the abort as an error event; body: {rendered}"
    );
    assert!(
        !rendered.contains("message_stop"),
        "an aborted turn must not claim a clean stop; body: {rendered}"
    );
    Ok(())
}

#[tokio::test]
async fn openai_chat_in_headless_stream_emits_error_chunk_and_done() -> anyhow::Result<()> {
    let rendered = run_headless("chat", InWire::OpenAiChat).await?;
    assert!(
        rendered.contains("\"type\":\"upstream_error\""),
        "the Chat Completions surface must state the abort as an error chunk; body: {rendered}"
    );
    assert!(
        rendered.contains("data: [DONE]"),
        "an OpenAI-SDK client reads until [DONE]; body: {rendered}"
    );
    Ok(())
}

#[tokio::test]
async fn openai_responses_in_headless_stream_emits_response_failed() -> anyhow::Result<()> {
    let rendered = run_headless("responses", InWire::OpenAiResponses).await?;
    assert!(
        rendered.contains("event: response.failed"),
        "the Responses surface must state the abort as response.failed; body: {rendered}"
    );
    assert!(
        rendered.contains("\"status\":\"failed\""),
        "a Responses client reads the outcome off response.status; body: {rendered}"
    );
    Ok(())
}
