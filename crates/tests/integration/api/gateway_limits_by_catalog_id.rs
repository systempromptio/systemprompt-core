//! The model card must still be found when the catalog id is not the upstream
//! name.
//!
//! `ProviderModel::matches` keys on the catalog id and its aliases, never on
//! `upstream_model`. The dispatch pipeline resolved `model_limits` with the
//! upstream name, so every provider whose ids differ from the upstream's --
//! both Vertex entries -- got `None` limits: no output-token clamp, no
//! thinking budget, no reasoning ceiling. Nothing failed loudly; the caller's
//! whole `max_tokens` simply went upstream unclamped, and Gemini 2.5 Pro was
//! left free to spend all of it thinking with no headroom for visible text.
//!
//! Each cell drives the real `GatewayService::dispatch` against a wiremock
//! upstream and asserts on the body the *upstream received*, which is the only
//! place the limits are observable.

use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
use serde_json::{Value, json};
use systemprompt_api::services::gateway::protocol::InboundAdapter;
use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_api::services::gateway::service::GatewayService;
use systemprompt_identifiers::{ModelId, ProviderId, RouteId, SecretName};
use systemprompt_models::services::ai::ModelLimits;
use systemprompt_models::services::{
    ApiSurface, GatewayConfig, GatewayRoute, ProviderEntry, ProviderModel, ProviderRegistry,
    WireProtocol,
};
use systemprompt_test_fixtures::seed_admin_credential;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;
use super::gateway_pipeline::{gw_repos, inputs_with, install_provider_api_key};

const PROVIDER: &str = "anthropic";
const API_KEY_SECRET: &str = "anthropic";
const CATALOG_ID: &str = "claude-catalog-id-model";
const UPSTREAM_NAME: &str = "vendor-upstream-name";
const CALLER_MAX_TOKENS: u32 = 4096;
const MODEL_CAP: u32 = 64;
const THINKING_BUDGET: u32 = 1024;
const GEMINI_CAP: u32 = 8192;

fn registry(
    endpoint: &str,
    wire: WireProtocol,
    surface: ApiSurface,
    limits: ModelLimits,
) -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![ProviderEntry {
            name: ProviderId::new(PROVIDER),
            wire,
            surface,
            endpoint: endpoint.to_owned(),
            api_key_secret: SecretName::new(API_KEY_SECRET),
            governance: Default::default(),
            extra_headers: HashMap::new(),
            models: vec![ProviderModel {
                id: ModelId::new(CATALOG_ID),
                aliases: Vec::new(),
                governance: None,
                upstream_model: Some(UPSTREAM_NAME.to_owned()),
                pricing: Default::default(),
                capabilities: Default::default(),
                limits,
            }],
        }],
    }
}

fn config() -> GatewayConfig {
    let mut route = GatewayRoute {
        id: RouteId::new(""),
        model_pattern: CATALOG_ID.to_owned(),
        provider: ProviderId::new(PROVIDER),
        upstream_model: None,
        extra_headers: HashMap::new(),
        pricing: None,
        when: None,
        requires: None,
    };
    route.ensure_id();
    GatewayConfig {
        enabled: true,
        routes: vec![route],
        ..GatewayConfig::default()
    }
}

fn caller_body() -> Bytes {
    Bytes::from(
        serde_json::to_vec(&json!({
            "model": CATALOG_ID,
            "max_tokens": CALLER_MAX_TOKENS,
            "messages": [{"role": "user", "content": "hello"}],
        }))
        .expect("serialize caller body"),
    )
}

fn openai_chat_reply() -> Value {
    json!({
        "id": "chatcmpl_limits",
        "object": "chat.completion",
        "model": UPSTREAM_NAME,
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "ok"},
            "finish_reason": "stop",
        }],
        "usage": {"prompt_tokens": 3, "completion_tokens": 1, "total_tokens": 4},
    })
}

fn gemini_reply() -> Value {
    json!({
        "candidates": [{
            "content": {"role": "model", "parts": [{"text": "ok"}]},
            "finishReason": "STOP",
        }],
        "usageMetadata": {"promptTokenCount": 3, "candidatesTokenCount": 1},
    })
}

// Why: returns the JSON body the upstream actually received, which is where a
// dropped model card shows up -- the caller's response looks identical either
// way.
async fn upstream_saw(
    label: &str,
    wire: WireProtocol,
    surface: ApiSurface,
    limits: ModelLimits,
    reply: Value,
) -> anyhow::Result<Value> {
    install_provider_api_key();
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, &format!("gw-limits-{label}@example.invalid")).await?;

    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(reply))
        .mount(&upstream)
        .await;

    let inbound: Arc<dyn InboundAdapter> = Arc::new(AnthropicMessagesInbound);
    let raw = caller_body();
    let request = inbound
        .parse_request(&raw)
        .map_err(|e| anyhow::anyhow!("inbound parse failed: {e}"))?;
    let di = inputs_with(&cred, request, false, inbound, raw);

    let resp = GatewayService::dispatch(
        &config(),
        &registry(&upstream.uri(), wire, surface, limits),
        &pool,
        &gw_repos(&pool),
        di,
    )
    .await
    .map_err(|e| anyhow::anyhow!("dispatch failed: {e:?}"))?;
    assert_eq!(resp.status(), http::StatusCode::OK, "cell {label}");

    let sent = upstream
        .received_requests()
        .await
        .and_then(|reqs| reqs.into_iter().next())
        .ok_or_else(|| anyhow::anyhow!("upstream received no request in cell {label}"))?;
    Ok(serde_json::from_slice(&sent.body)?)
}

const fn clamp_only() -> ModelLimits {
    ModelLimits {
        context_window: 200_000,
        max_output_tokens: MODEL_CAP,
        max_thinking_budget: None,
    }
}

const fn with_thinking_budget() -> ModelLimits {
    ModelLimits {
        context_window: 1_048_576,
        max_output_tokens: GEMINI_CAP,
        max_thinking_budget: Some(THINKING_BUDGET),
    }
}

#[tokio::test]
async fn openai_chat_clamps_output_tokens_to_the_card_found_by_catalog_id() -> anyhow::Result<()> {
    let body = upstream_saw(
        "chat-clamp",
        WireProtocol::OpenAiChat,
        ApiSurface::OpenAi,
        clamp_only(),
        openai_chat_reply(),
    )
    .await?;

    assert_eq!(
        body["max_completion_tokens"].as_u64(),
        Some(u64::from(MODEL_CAP)),
        "the caller asked for {CALLER_MAX_TOKENS}; the card caps it at {MODEL_CAP}. \
         Seeing the caller's number back means the card was not found: {body}"
    );
    assert_eq!(
        body["model"].as_str(),
        Some(UPSTREAM_NAME),
        "the upstream still gets its own name for the model: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn openai_chat_uses_the_reasoning_ceiling_from_the_card() -> anyhow::Result<()> {
    let limits = ModelLimits {
        max_thinking_budget: Some(2048),
        ..clamp_only()
    };
    let body = upstream_saw(
        "chat-reasoning",
        WireProtocol::OpenAiChat,
        ApiSurface::OpenAi,
        limits,
        openai_chat_reply(),
    )
    .await?;

    // A card carrying a thinking budget marks a reasoning model, whose whole
    // completion budget is the card cap rather than the caller's number.
    assert_eq!(
        body["max_completion_tokens"].as_u64(),
        Some(u64::from(MODEL_CAP)),
        "reasoning ceiling must come from the card, not the caller: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn gemini_budgets_thinking_from_the_card_found_by_catalog_id() -> anyhow::Result<()> {
    let body = upstream_saw(
        "gemini-thinking",
        WireProtocol::Gemini,
        ApiSurface::Gemini,
        with_thinking_budget(),
        gemini_reply(),
    )
    .await?;

    let cfg = &body["generationConfig"];
    // The card's budget buys headroom, never a thinkingConfig: sending one
    // would switch thinking on for models Google ships with it off.
    assert!(
        cfg.get("thinkingConfig").is_none(),
        "the implicit path must send no thinkingConfig: {body}"
    );
    assert_eq!(
        cfg["maxOutputTokens"].as_u64(),
        Some(u64::from(CALLER_MAX_TOKENS + THINKING_BUDGET)),
        "the ceiling is raised by the card's thinking budget; the caller's own \
         number back means the card was never found: {body}"
    );
    Ok(())
}
