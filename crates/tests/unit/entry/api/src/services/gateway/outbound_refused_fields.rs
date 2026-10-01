//! An upstream that refuses a request body field is answered without it.
//!
//! The real `AnthropicOutbound` adapter against a wiremock upstream that 400s
//! with Vertex AI's wording — `context_management: Extra inputs are not
//! permitted` — whenever the body carries the field, and answers otherwise:
//! the request succeeds after one re-send, the provider remembers the field
//! for its next request, and a field the Messages API requires is never
//! dropped however the upstream phrases its refusal.

use std::collections::{BTreeSet, HashMap};

use serde_json::{Value, json};
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_api::services::gateway::protocol::outbound::anthropic::AnthropicOutbound;
use systemprompt_api::services::gateway::protocol::outbound::anthropic::refused_fields::{
    learned, refused_in,
};
use systemprompt_api::services::gateway::protocol::outbound::retry::{RetryPolicy, with_policy};
use systemprompt_api::services::gateway::protocol::outbound::{
    OutboundAdapter, OutboundCtx, OutboundError, OutboundOutcome,
};
use systemprompt_identifiers::{ModelId, ProviderId, RouteId};
use systemprompt_models::services::GatewayRoute;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

use super::support;

const REFUSED_FIELD: &str = "context_management";

fn route(provider: &str) -> GatewayRoute {
    GatewayRoute {
        id: Some(RouteId::new("r1"))"r1")),
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
        tool_choice: None,
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

// A client body carrying the field with no flag: the gateway has no gate to
// close, so only the upstream's refusal can teach it.
fn raw_body() -> bytes::Bytes {
    bytes::Bytes::from(
        serde_json::to_vec(&json!({
            "model": "upstream-1",
            "max_tokens": 64,
            "messages": [{ "role": "user", "content": "hi" }],
            REFUSED_FIELD: { "edits": [{ "type": "clear_thinking_20251015", "keep": "all" }] },
            "output_config": { "effort": "medium" }
        }))
        .expect("serialize"),
    )
}

struct RefusesField;

impl Respond for RefusesField {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&req.body).expect("json body");
        if body.get(REFUSED_FIELD).is_some() {
            return ResponseTemplate::new(400).set_body_json(json!({
                "type": "error",
                "error": {
                    "type": "invalid_request_error",
                    "message": format!("{REFUSED_FIELD}: Extra inputs are not permitted")
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

async fn send(endpoint: &str, provider: &str) -> Result<OutboundOutcome, OutboundError> {
    let route = route(provider);
    let req = request();
    let raw = raw_body();
    let ctx = OutboundCtx {
        route: &route,
        upstream: &support::api_key_call(endpoint, "k"),
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        automatic_prompt_caching: false,
        forward_headers: &[],
        raw_body: Some(&raw),
    };
    let body = AnthropicOutbound.build_body(&ctx)?;
    assert!(body.raw_lane);
    with_policy(RetryPolicy::immediate(), AnthropicOutbound.send(ctx, &body)).await
}

fn bodies_sent(requests: &[Request]) -> Vec<Value> {
    requests
        .iter()
        .map(|r| serde_json::from_slice(&r.body).expect("json"))
        .collect()
}

#[tokio::test]
async fn a_refused_field_is_dropped_and_the_request_resent_once() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(RefusesField)
        .mount(&server)
        .await;

    let outcome = send(&server.uri(), "refusing-field-upstream-a").await;

    assert!(matches!(
        outcome.expect("recovered"),
        OutboundOutcome::RawBuffered { .. }
    ));
    let sent = bodies_sent(&server.received_requests().await.expect("log"));
    assert_eq!(sent.len(), 2, "one refusal, one re-send");
    assert!(sent[0].get(REFUSED_FIELD).is_some());
    assert!(sent[1].get(REFUSED_FIELD).is_none());
    assert_eq!(
        sent[1]["output_config"]["effort"], "medium",
        "only the refused field is removed"
    );
}

#[tokio::test]
async fn the_provider_remembers_the_refused_field_for_its_next_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(RefusesField)
        .mount(&server)
        .await;

    send(&server.uri(), "refusing-field-upstream-b")
        .await
        .expect("first");
    send(&server.uri(), "refusing-field-upstream-b")
        .await
        .expect("second");

    assert!(learned("refusing-field-upstream-b").contains(REFUSED_FIELD));
    let sent = bodies_sent(&server.received_requests().await.expect("log"));
    assert_eq!(
        sent.len(),
        3,
        "the second request goes out without the field, no refusal"
    );
    assert!(sent[2].get(REFUSED_FIELD).is_none());
}

#[tokio::test]
async fn an_unrelated_400_is_returned_as_is() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "type": "error",
            "error": { "type": "invalid_request_error", "message": "max_tokens: must be positive" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    assert!(send(&server.uri(), "strict-upstream").await.is_err());
    assert!(learned("strict-upstream").is_empty());
}

#[test]
fn refusal_messages_name_the_top_level_field() {
    assert_eq!(
        refused_in("context_management: Extra inputs are not permitted"),
        BTreeSet::from(["context_management".to_owned()])
    );
    assert_eq!(
        refused_in("mcp_servers.0.authorization_token: Extra inputs are not permitted"),
        BTreeSet::from(["mcp_servers".to_owned()]),
        "a nested path names its top-level key"
    );
    assert_eq!(
        refused_in(
            "context_management: Extra inputs are not permitted; container: Extra inputs are not permitted"
        ),
        BTreeSet::from(["context_management".to_owned(), "container".to_owned()])
    );
    assert!(refused_in("max_tokens: must be positive").is_empty());
    assert!(
        refused_in("messages: Extra inputs are not permitted").is_empty(),
        "a required field is never dropped, however the upstream phrases it"
    );
    assert!(refused_in("Extra inputs are not permitted").is_empty());
}
