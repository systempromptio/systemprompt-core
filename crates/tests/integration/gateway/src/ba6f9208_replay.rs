//! Production request `ba6f9208` end to end: an `OpenCode` Chat Completions
//! body with two parallel tool calls answered by two `role: "tool"` messages,
//! parsed by the real inbound adapter and sent by the real Anthropic outbound
//! adapter to a recording upstream. `DeepSeek`'s Anthropic-compatible endpoint
//! rejected the original with "tool_use ids were found without tool_result
//! blocks immediately after"; the body it receives must now answer both calls
//! in the one user turn that follows them.

use std::sync::{Arc, Mutex};

use axum::Json;
use axum::extract::State;
use axum::routing::post;
use bytes::Bytes;
use serde_json::{Value, json};
use systemprompt_ai::UpstreamTarget;
use systemprompt_catalog_fixtures::{CatalogFixture, model, platform, priced};
use systemprompt_gateway::GatewayUpstreamRegistry;
use systemprompt_gateway::protocol::inbound::InboundAdapter;
use systemprompt_gateway::protocol::inbound::openai_chat::OpenAiChatInbound;
use systemprompt_gateway::protocol::outbound::{OutboundCtx, OutboundOutcome};
use systemprompt_manifest::services::catalog::{EndpointConfig, ProviderConfig, ResolvedCatalog};
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::WireProtocol;

type Captured = Arc<Mutex<Option<Value>>>;

fn opencode_body() -> Value {
    json!({
        "model": "deepseek-flash",
        "stream": false,
        "max_tokens": 1024,
        "messages": [
            { "role": "system", "content": "You are opencode." },
            { "role": "user", "content": "Read both files." },
            { "role": "assistant", "content": "", "tool_calls": [
                { "id": "toolu_p1", "type": "function",
                  "function": { "name": "read", "arguments": "{\"filePath\":\"a.rs\"}" } },
                { "id": "toolu_p2", "type": "function",
                  "function": { "name": "read", "arguments": "{\"filePath\":\"b.rs\"}" } }
            ] },
            { "role": "tool", "tool_call_id": "toolu_p1", "content": "fn a() {}" },
            { "role": "tool", "tool_call_id": "toolu_p2", "content": "fn b() {}" }
        ],
        "tools": [{ "type": "function", "function": {
            "name": "read", "parameters": { "type": "object" } } }]
    })
}

async fn record(State(captured): State<Captured>, Json(body): Json<Value>) -> Json<Value> {
    *captured.lock().expect("capture lock") = Some(body);
    Json(json!({
        "id": "msg_1", "type": "message", "role": "assistant", "model": "deepseek-flash",
        "content": [{ "type": "text", "text": "a defines a, b defines b" }],
        "stop_reason": "end_turn", "stop_sequence": null,
        "usage": { "input_tokens": 40, "output_tokens": 8 }
    }))
}

async fn recording_upstream() -> (String, Captured) {
    let captured: Captured = Arc::new(Mutex::new(None));
    let app = axum::Router::new()
        .route("/anthropic/v1/messages", post(record))
        .with_state(Arc::clone(&captured));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    (format!("http://{addr}/anthropic/v1"), captured)
}

fn deepseek(endpoint: &str) -> ResolvedCatalog {
    CatalogFixture::new()
        .credential("deepseek", platform("deepseek"))
        .provider("deepseek", ProviderConfig::new("deepseek"))
        .endpoint(
            "deepseek",
            "messages",
            EndpointConfig::new(WireProtocol::Anthropic, endpoint),
        )
        .model(
            "deepseek-flash",
            model(&[ClientFamily::OpenAi], 0, priced(1.0, 1.0, 0.0)),
        )
        .deploy("deepseek-flash", ["deepseek/messages"])
        .build()
}

#[tokio::test]
async fn opencode_parallel_tool_results_reach_an_anthropic_wire_upstream_as_one_turn() {
    let (endpoint, captured) = recording_upstream().await;
    let catalog = deepseek(&endpoint);
    let hop = catalog.deployments("deepseek-flash")[0].clone();
    let call = UpstreamTarget::resolve(&catalog, &hop, "sk-test")
        .expect("target")
        .call()
        .await
        .expect("call");
    let raw = Bytes::from(serde_json::to_vec(&opencode_body()).expect("body"));
    let request = OpenAiChatInbound.parse_request(&raw).expect("parse");
    let ctx = OutboundCtx {
        target: &hop,
        upstream: &call,
        request: &request,
        upstream_model: "deepseek-flash",
        model_limits: None,
        automatic_prompt_caching: false,
        forward_headers: &[],
        // Chat inbound to an anthropic-wire provider never takes the raw lane.
        raw_body: None,
    };
    let upstreams = GatewayUpstreamRegistry::new();
    let adapter = upstreams.get("anthropic").expect("anthropic adapter");
    let body = adapter.build_body(&ctx).expect("build body");
    let outcome = adapter.send(ctx, &body).await.expect("send");
    assert!(matches!(outcome, OutboundOutcome::Buffered(_)));

    let sent = captured
        .lock()
        .expect("capture lock")
        .take()
        .expect("upstream received the request");
    let messages = sent["messages"].as_array().expect("messages");
    let roles: Vec<&str> = messages
        .iter()
        .map(|m| m["role"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(roles, ["user", "assistant", "user"], "{sent:#}");
    assert_eq!(sent["system"], "You are opencode.");
    let calls: Vec<&str> = messages[1]["content"]
        .as_array()
        .expect("assistant content")
        .iter()
        .filter_map(|b| b["id"].as_str())
        .collect();
    assert_eq!(calls, ["toolu_p1", "toolu_p2"]);
    let answers: Vec<&str> = messages[2]["content"]
        .as_array()
        .expect("user content")
        .iter()
        .map(|b| {
            assert_eq!(b["type"], "tool_result", "{sent:#}");
            b["tool_use_id"].as_str().unwrap_or_default()
        })
        .collect();
    assert_eq!(answers, ["toolu_p1", "toolu_p2"]);
}
