//! An upstream that refuses an `anthropic-beta` value is answered without it.
//!
//! The real `AnthropicOutbound` adapter against a wiremock upstream that 400s
//! with Anthropic's own refusal text whenever the header names the beta, and
//! answers otherwise: the request succeeds after one re-send, the kept betas
//! survive, and the provider remembers the refusal for its next request.

use std::collections::{BTreeSet, HashMap};

use serde_json::json;
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalToolChoice, Role,
};
use systemprompt_api::services::gateway::protocol::outbound::anthropic::AnthropicOutbound;
use systemprompt_api::services::gateway::protocol::outbound::anthropic::rejected_betas::{
    learned, refused_in, without,
};
use systemprompt_api::services::gateway::protocol::outbound::retry::{RetryPolicy, with_policy};
use systemprompt_api::services::gateway::protocol::outbound::{
    OutboundAdapter, OutboundCtx, OutboundOutcome,
};
use systemprompt_identifiers::{ModelId, ProviderId, RouteId};
use systemprompt_models::services::GatewayRoute;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

use super::support;

const REFUSED: &str = "advisor-tool-2026-03-01";
const KEPT: &str = "interleaved-thinking-2025-05-14";

fn route(provider: &str) -> GatewayRoute {
    GatewayRoute {
        id: RouteId::new("r1"),
        name: None,
        description: None,
        model_pattern: "*".into(),
        provider: ProviderId::new(provider),
        upstream_model: Some("upstream-1".into()),
        extra_headers: HashMap::new(),
        pricing: None,
        when: None,
        requires: None,
        fallback_provider: None,
        fallback_upstream_model: None,
    }
}

fn request() -> CanonicalRequest {
    CanonicalRequest {
        model: ModelId::new("m"),
        cache_control: None,
        system: Vec::new(),
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::text("hi")],
        }],
        max_tokens: 64,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: vec![],
        tools: vec![],
        tool_choice: None::<CanonicalToolChoice>,
        stream: false,
        thinking: None,
        metadata: None,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
        forwarded_surface: Default::default(),
    }
}

struct RefusesBeta;

impl Respond for RefusesBeta {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        let betas = req
            .headers
            .get("anthropic-beta")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if betas.contains(REFUSED) {
            return ResponseTemplate::new(400).set_body_json(json!({
                "type": "error",
                "error": {
                    "type": "invalid_request_error",
                    "message": format!("Unexpected value(s) `{REFUSED}` for the `anthropic-beta` header. Please consult our documentation at platform.claude.com/docs or try again without the header.")
                }
            }));
        }
        ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg_ok",
            "type": "message",
            "role": "assistant",
            "model": "upstream-1",
            "content": [{ "type": "text", "text": "ok" }],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 1, "output_tokens": 1 }
        }))
    }
}

async fn send(endpoint: &str, provider: &str) -> anyhow::Result<OutboundOutcome> {
    let route = route(provider);
    let req = request();
    let forward = vec![("anthropic-beta".to_owned(), format!("{REFUSED},{KEPT}"))];
    let ctx = OutboundCtx {
        route: &route,
        upstream: &support::api_key_call(endpoint, "k"),
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        automatic_prompt_caching: false,
        forward_headers: &forward,
        raw_body: None,
    };
    let body = AnthropicOutbound.build_body(&ctx)?;
    with_policy(RetryPolicy::immediate(), AnthropicOutbound.send(ctx, &body)).await
}

fn betas_sent(requests: &[Request]) -> Vec<String> {
    requests
        .iter()
        .map(|r| {
            r.headers
                .get("anthropic-beta")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

#[tokio::test]
async fn a_refused_beta_is_dropped_and_the_request_resent_once() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(RefusesBeta)
        .mount(&server)
        .await;

    let outcome = send(&server.uri(), "refusing-upstream-a").await;

    assert!(matches!(
        outcome.expect("recovered"),
        OutboundOutcome::Buffered(_)
    ));
    let sent = betas_sent(&server.received_requests().await.expect("log"));
    assert_eq!(sent, vec![format!("{REFUSED},{KEPT}"), KEPT.to_owned()]);
}

#[tokio::test]
async fn the_provider_remembers_the_refusal_for_its_next_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(RefusesBeta)
        .mount(&server)
        .await;

    send(&server.uri(), "refusing-upstream-b")
        .await
        .expect("first");
    send(&server.uri(), "refusing-upstream-b")
        .await
        .expect("second");

    assert!(learned("refusing-upstream-b").contains(REFUSED));
    let sent = betas_sent(&server.received_requests().await.expect("log"));
    assert_eq!(
        sent.len(),
        3,
        "the second request goes out without the refused beta"
    );
    assert_eq!(sent[2], KEPT);
}

#[test]
fn refusal_messages_name_every_value() {
    let one = refused_in("Unexpected value(s) `a-2026` for the `anthropic-beta` header.");
    assert_eq!(one, BTreeSet::from(["a-2026".to_owned()]));
    let many =
        refused_in("Unexpected value(s) `a-2026`, `b-2026` for the `anthropic-beta` header.");
    assert_eq!(
        many,
        BTreeSet::from(["a-2026".to_owned(), "b-2026".to_owned()])
    );
    assert!(refused_in("max_tokens: must be positive").is_empty());
}

#[test]
fn dropping_every_value_drops_the_header() {
    let drop = BTreeSet::from(["a".to_owned()]);
    let kept = without(vec![("anthropic-beta".to_owned(), "a".to_owned())], &drop);
    assert!(kept.is_empty());
}
