//! A hop served with the credential the client presented is sent as it
//! arrived.
//!
//! The body keeps the client's bytes, taking only the route's upstream model
//! id. Through the real `AnthropicOutbound` adapter against a wiremock
//! upstream, the client's bearer and beta flags reach the upstream whole, and
//! an upstream refusal is returned once and unlearned.

use serde_json::{Value, json};
use systemprompt_ai::UpstreamCall;
use systemprompt_gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_gateway::protocol::outbound::anthropic::AnthropicOutbound;
use systemprompt_gateway::protocol::outbound::anthropic::refused_fields::learned;
use systemprompt_gateway::protocol::outbound::retry::{RetryPolicy, with_policy};
use systemprompt_gateway::protocol::outbound::{
    OutboundAdapter, OutboundCtx, OutboundError, OutboundOutcome, PreparedBody,
};
use systemprompt_gateway::service::stages::as_sent_body;
use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::catalog::{CredentialKind, ResolvedHop};
use systemprompt_security::credential::{AuthHeader, AuthScheme};
use systemprompt_wire::Hosting;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::support;

const REFUSED_FIELD: &str = "context_management";
const BETAS: &str = "client-capability-a,client-capability-b";

fn caller_hop(provider: &str) -> ResolvedHop {
    let mut hop = systemprompt_catalog_fixtures::hop(
        provider,
        systemprompt_wire::WireProtocol::Anthropic,
        "https://upstream.invalid/v1",
        "m",
        "upstream-1",
    );
    hop.credential.kind = CredentialKind::Caller;
    hop.credential.secret = None;
    hop
}

fn caller_call(endpoint: &str) -> UpstreamCall {
    UpstreamCall::new(
        Hosting::FirstParty,
        endpoint.to_owned(),
        AuthHeader {
            scheme: AuthScheme::Bearer,
            value: "client-credential".to_owned(),
        },
        Vec::new(),
    )
    .with_presented_headers(vec![
        ("user-agent".to_owned(), "client/1.0".to_owned()),
        ("anthropic-beta".to_owned(), BETAS.to_owned()),
    ])
}

fn request() -> CanonicalRequest {
    CanonicalRequest {
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::text("hi")],
        }],
        ..CanonicalRequest::new(ModelId::new("m"), Vec::new(), 64)
    }
}

fn raw_body(model: &str) -> bytes::Bytes {
    bytes::Bytes::from(
        serde_json::to_vec(&json!({
            "model": model,
            "max_tokens": 1_000_000,
            "metadata": { "user_id": "client-user" },
            "messages": [{ "role": "user", "content": "hi" }],
            REFUSED_FIELD: { "edits": [] }
        }))
        .expect("serialize"),
    )
}

async fn send(
    endpoint: &str,
    provider: &str,
    raw: &bytes::Bytes,
) -> Result<OutboundOutcome, OutboundError> {
    let hop = caller_hop(provider);
    let req = request();
    let forward = [("x-added-by-the-gateway".to_owned(), "1".to_owned())];
    let ctx = OutboundCtx {
        target: &hop,
        policy: systemprompt_gateway::service::HopPolicy::of(&hop),
        upstream: &caller_call(endpoint),
        http: &support::upstream_http(),
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        automatic_prompt_caching: true,
        forward_headers: &forward,
        raw_body: Some(raw),
    };
    let body = PreparedBody {
        bytes: as_sent_body(raw, "upstream-1").expect("a json body"),
        raw_lane: true,
    };
    let traced = systemprompt_gateway::protocol::outbound::trace_context::with_trace(
        Some(systemprompt_identifiers::TraceId::generate()),
        Some(true),
        AnthropicOutbound.send(ctx, &body),
    );
    with_policy(RetryPolicy::immediate(), traced).await
}

#[test]
fn a_body_naming_the_upstream_model_keeps_its_bytes() {
    let raw = raw_body("upstream-1");
    assert_eq!(as_sent_body(&raw, "upstream-1"), Some(raw));
}

#[test]
fn the_upstream_model_id_is_the_only_edit_a_body_takes() {
    let sent: Value =
        serde_json::from_slice(&as_sent_body(&raw_body("alias"), "upstream-1").expect("json"))
            .expect("json");
    let mut expected: Value = serde_json::from_slice(&raw_body("alias")).expect("json");
    expected["model"] = json!("upstream-1");
    assert_eq!(sent, expected);
}

#[test]
fn a_body_that_is_not_a_json_object_is_not_sent_whole() {
    assert_eq!(as_sent_body(&bytes::Bytes::from_static(b"[1]"), "m"), None);
}

#[tokio::test]
async fn the_clients_body_bearer_and_betas_reach_the_upstream_whole() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg_ok",
            "type": "message",
            "role": "assistant",
            "model": "upstream-1",
            "content": [{ "type": "text", "text": "ok" }],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 1, "output_tokens": 1 }
        })))
        .mount(&server)
        .await;
    let raw = raw_body("upstream-1");

    send(&server.uri(), "caller-upstream-whole", &raw)
        .await
        .expect("served");

    let requests = server.received_requests().await.expect("log");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].body, raw.to_vec());
    let header = |name: &str| {
        requests[0]
            .headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    assert_eq!(
        header("authorization").as_deref(),
        Some("Bearer client-credential")
    );
    assert_eq!(header("anthropic-beta").as_deref(), Some(BETAS));
    assert_eq!(header("user-agent").as_deref(), Some("client/1.0"));
    for withheld in ["x-api-key", "traceparent", "x-added-by-the-gateway"] {
        assert!(header(withheld).is_none(), "{withheld} is not the client's");
    }
}

#[tokio::test]
async fn an_upstream_refusal_is_returned_once_and_not_learned() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "type": "error",
            "error": {
                "type": "invalid_request_error",
                "message": format!("{REFUSED_FIELD}: Extra inputs are not permitted")
            }
        })))
        .mount(&server)
        .await;
    let provider = "caller-upstream-refusing";

    let refused = send(&server.uri(), provider, &raw_body("upstream-1")).await;

    assert!(matches!(refused, Err(OutboundError::Upstream(_))));
    assert_eq!(
        server.received_requests().await.expect("log").len(),
        1,
        "no re-send without the refused field"
    );
    let key = caller_hop(provider).endpoint_ref().to_string();
    assert!(learned(&key).is_empty(), "one member's refusal is not kept");
}
