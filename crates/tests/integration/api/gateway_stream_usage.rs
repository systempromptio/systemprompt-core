//! Where a streamed Chat Completions turn reports its token counts.
//!
//! `stream_options.include_usage` is what OpenCode and the OpenAI SDKs send,
//! and the contract answers it with a usage-only chunk after the finish chunk
//! and before `[DONE]`. The gateway used to answer it on the finish chunk
//! instead, rendered before the upstream's usage had arrived, so every count
//! the caller read was zero while the audit row for the same request carried
//! the real ones. These cells drive a real dispatch and read the counts off
//! the frames the caller actually receives.

use std::sync::Arc;

use axum::body::to_bytes;
use bytes::Bytes;
use serde_json::{Value, json};
use systemprompt_api::services::gateway::protocol::InboundAdapter;
use systemprompt_api::services::gateway::protocol::inbound::openai_chat::OpenAiChatInbound;
use systemprompt_api::services::gateway::service::GatewayService;
use systemprompt_models::services::{ApiSurface, WireProtocol};
use systemprompt_test_fixtures::seed_admin_credential;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;
use super::gateway_pipeline::{
    MODEL, PROVIDER, gateway_config, gw_repos, inputs_with, install_provider_api_key,
    provider_registry,
};

// Why: the counts arrive on `message_delta`, after the content and with the
// terminal reason -- the same late position Chat Completions puts them in.
fn upstream_sse() -> String {
    [
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_m\",\"model\":\"claude-test-model\",\"usage\":{\"input_tokens\":11,\"output_tokens\":0}}}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":18}}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    ]
    .concat()
}

fn chat_body(include_usage: bool) -> Bytes {
    let mut body = json!({
        "model": MODEL,
        "stream": true,
        "messages": [{"role": "user", "content": "hi"}],
    });
    if include_usage {
        body["stream_options"] = json!({ "include_usage": true });
    }
    Bytes::from(serde_json::to_vec(&body).unwrap_or_default())
}

async fn run(label: &str, include_usage: bool) -> anyhow::Result<String> {
    install_provider_api_key();
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, &format!("gw-usage-{label}@example.invalid")).await?;

    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(upstream_sse(), "text/event-stream"),
        )
        .mount(&upstream)
        .await;

    let adapter: Arc<dyn InboundAdapter> = Arc::new(OpenAiChatInbound);
    let raw = chat_body(include_usage);
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
    let di = inputs_with(&cred, request, true, adapter, raw);

    let resp = GatewayService::dispatch(&config, &registry, &pool, &gw_repos(&pool), di)
        .await
        .map_err(|e| anyhow::anyhow!("dispatch failed: {e:?}"))?;
    let bytes = to_bytes(resp.into_body(), 4 * 1024 * 1024).await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn usage_chunk(rendered: &str) -> Option<Value> {
    rendered
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter(|data| data.trim() != "[DONE]")
        .filter_map(|data| serde_json::from_str::<Value>(data).ok())
        .find(|v| v.get("usage").is_some_and(|u| !u.is_null()))
}

#[tokio::test]
async fn include_usage_gets_a_trailing_chunk_with_the_real_counts() -> anyhow::Result<()> {
    let rendered = run("asked", true).await?;
    let chunk = usage_chunk(&rendered)
        .ok_or_else(|| anyhow::anyhow!("no usage chunk; body: {rendered}"))?;
    assert_eq!(chunk["usage"]["completion_tokens"], 18, "{rendered}");
    assert_eq!(chunk["usage"]["prompt_tokens"], 11, "{rendered}");
    assert_eq!(
        chunk["choices"].as_array().map(Vec::len),
        Some(0),
        "the contract's usage chunk carries no choices; {rendered}"
    );
    Ok(())
}

#[tokio::test]
async fn the_usage_chunk_lands_between_the_finish_chunk_and_the_sentinel() -> anyhow::Result<()> {
    let rendered = run("ordered", true).await?;
    let finish = rendered
        .find("\"finish_reason\":\"stop\"")
        .ok_or_else(|| anyhow::anyhow!("no finish chunk; body: {rendered}"))?;
    let usage = rendered
        .find("\"completion_tokens\":18")
        .ok_or_else(|| anyhow::anyhow!("no usage chunk; body: {rendered}"))?;
    let done = rendered
        .find("[DONE]")
        .ok_or_else(|| anyhow::anyhow!("no sentinel; body: {rendered}"))?;
    assert!(finish < usage && usage < done, "{rendered}");
    Ok(())
}

#[tokio::test]
async fn a_caller_that_did_not_ask_gets_no_usage_at_all() -> anyhow::Result<()> {
    let rendered = run("silent", false).await?;
    assert!(
        usage_chunk(&rendered).is_none(),
        "usage is reported only when asked for; body: {rendered}"
    );
    assert!(rendered.contains("data: [DONE]"), "{rendered}");
    Ok(())
}
