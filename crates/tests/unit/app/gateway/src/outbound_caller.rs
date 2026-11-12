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

async fn send_chat(endpoint: &str, kind: CredentialKind) -> Result<OutboundOutcome, OutboundError> {
    use systemprompt_gateway::protocol::outbound::openai_chat::OpenAiChatOutbound;
    let mut hop = systemprompt_catalog_fixtures::hop(
        "chat-upstream",
        systemprompt_wire::WireProtocol::OpenAiChat,
        "https://upstream.invalid/v1",
        "m",
        "upstream-1",
    );
    hop.credential.kind = kind;
    let req = request();
    let raw = bytes::Bytes::from_static(
        br#"{"model":"upstream-1","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let ctx = OutboundCtx {
        target: &hop,
        policy: systemprompt_gateway::service::HopPolicy::of(&hop),
        upstream: &caller_call(endpoint),
        http: &support::upstream_http(),
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        automatic_prompt_caching: false,
        forward_headers: &[],
        raw_body: Some(&raw),
    };
    let body = PreparedBody {
        bytes: raw.clone(),
        raw_lane: true,
    };
    let traced = systemprompt_gateway::protocol::outbound::trace_context::with_trace(
        Some(systemprompt_identifiers::TraceId::generate()),
        Some(true),
        OpenAiChatOutbound.send(ctx, &body),
    );
    with_policy(RetryPolicy::immediate(), traced).await
}

async fn overloaded(status: u16, route: &str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(route))
        .respond_with(
            ResponseTemplate::new(status)
                .insert_header("retry-after", "0")
                .set_body_json(json!({ "error": { "message": "slow down" } })),
        )
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn a_rate_limited_caller_hop_is_sent_once_on_the_anthropic_wire() {
    let server = overloaded(429, "/messages").await;

    let refused = send(
        &server.uri(),
        "caller-upstream-limited",
        &raw_body("upstream-1"),
    )
    .await;

    match refused {
        Err(OutboundError::Upstream(error)) => {
            assert!(error.to_string().contains("429"), "{error}");
        },
        _ => panic!("the upstream's own 429 is the answer"),
    }
    assert_eq!(server.received_requests().await.expect("log").len(), 1);
}

#[tokio::test]
async fn a_caller_hop_is_sent_once_and_untraced_on_the_openai_chat_wire() {
    let server = overloaded(503, "/chat/completions").await;

    let refused = send_chat(&server.uri(), CredentialKind::Caller).await;

    assert!(matches!(refused, Err(OutboundError::Upstream(_))));
    let received = server.received_requests().await.expect("log");
    assert_eq!(received.len(), 1, "no gateway retry on the member's limit");
    assert!(
        received[0].headers.get("traceparent").is_none(),
        "the request is not part of this platform's trace"
    );
}

#[tokio::test]
async fn a_governed_hop_on_the_same_wire_is_retried_and_traced() {
    let server = overloaded(503, "/chat/completions").await;

    let refused = send_chat(&server.uri(), CredentialKind::Platform).await;

    assert!(matches!(refused, Err(OutboundError::Upstream(_))));
    let received = server.received_requests().await.expect("log");
    assert_eq!(
        received.len(),
        usize::try_from(RetryPolicy::immediate().max_attempts).expect("fits"),
        "the control: the same send under a platform credential spends the retry budget"
    );
    assert!(received[0].headers.get("traceparent").is_some());
}

mod relayed {
    use futures_util::StreamExt;
    use systemprompt_gateway::protocol::outbound::openai_chat::OpenAiChatOutbound;
    use systemprompt_gateway::protocol::outbound::{RelayedBody, UpstreamError};
    use systemprompt_wire::WireProtocol;
    use systemprompt_wire::upstream::is_returned_response_header;

    use super::*;

    const ANTHROPIC_BODY: &str = "{ \"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\
        \"model\":\"upstream-1\",\n  \"content\":[{\"type\":\"tool_use\",\"id\":\"tu_1\",\
        \"name\":\"do\",\"input\":{}}],\"stop_reason\":\"end_turn\",\"vendor_extra\":{\"k\":1},\
        \"usage\":{\"input_tokens\":7,\"output_tokens\":3} }";
    const CHAT_BODY: &str = "{\"id\":\"chatcmpl-1\",\"object\":\"chat.completion\",\
        \"system_fingerprint\":\"fp_abc\",\"model\":\"upstream-1\",\"choices\":[{\"index\":0,\
        \"logprobs\":null,\"message\":{\"role\":\"assistant\",\"content\":\"ok\"},\
        \"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2,\
        \"total_tokens\":7}}";
    const CHAT_STREAM: &str = ": keep-alive\n\ndata: {\"id\":\"c1\",\"model\":\"upstream-1\",\
        \"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"ok\"}}]}\n\n\
        data: {\"vendor_only\":true}\n\ndata: {\"id\":\"c1\",\"model\":\"upstream-1\",\
        \"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\
        \"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2,\"total_tokens\":7}}\n\n\
        data: [DONE]\n\n";

    fn hop_on(wire: WireProtocol, kind: CredentialKind) -> ResolvedHop {
        let mut hop = systemprompt_catalog_fixtures::hop(
            "relay-upstream",
            wire,
            "https://upstream.invalid/v1",
            "m",
            "upstream-1",
        );
        hop.credential.kind = kind;
        hop
    }

    async fn exchange(
        adapter: &dyn OutboundAdapter,
        hop: &ResolvedHop,
        endpoint: &str,
        stream: bool,
    ) -> Result<OutboundOutcome, OutboundError> {
        let req = CanonicalRequest {
            stream,
            ..request()
        };
        let raw = bytes::Bytes::from_static(
            br#"{"model":"upstream-1","max_tokens":64,"messages":[{"role":"user","content":"hi"}]}"#,
        );
        let ctx = OutboundCtx {
            target: hop,
            policy: systemprompt_gateway::service::HopPolicy::of(hop),
            upstream: &caller_call(endpoint),
            http: &support::upstream_http(),
            request: &req,
            upstream_model: "upstream-1",
            model_limits: None,
            automatic_prompt_caching: false,
            forward_headers: &[],
            raw_body: Some(&raw),
        };
        let body = PreparedBody {
            bytes: raw.clone(),
            raw_lane: true,
        };
        with_policy(RetryPolicy::immediate(), adapter.send(ctx, &body)).await
    }

    async fn upstream(route: &str, template: ResponseTemplate) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(route))
            .respond_with(
                template
                    .insert_header("anthropic-ratelimit-requests-remaining", "41")
                    .insert_header("request-id", "req_upstream_1")
                    .insert_header("set-cookie", "__cf_bm=1; Path=/")
                    .insert_header("x-systemprompt-request-id", "spoofed"),
            )
            .mount(&server)
            .await;
        server
    }

    fn relayed(outcome: OutboundOutcome) -> systemprompt_gateway::protocol::outbound::Relayed {
        match outcome {
            OutboundOutcome::Relayed(answer) => answer,
            _ => panic!("a hop served as presented returns the upstream answer whole"),
        }
    }

    fn assert_upstream_headers(headers: &http::HeaderMap) {
        assert_eq!(
            headers
                .get("anthropic-ratelimit-requests-remaining")
                .and_then(|v| v.to_str().ok()),
            Some("41")
        );
        assert_eq!(
            headers.get("request-id").and_then(|v| v.to_str().ok()),
            Some("req_upstream_1")
        );
        assert!(headers.get("content-type").is_some());
        assert!(headers.get("set-cookie").is_none());
        assert!(headers.get("content-length").is_none());
        assert!(headers.get("x-systemprompt-request-id").is_none());
    }

    #[tokio::test]
    async fn an_anthropic_answer_keeps_its_bytes_headers_and_stop_reason() {
        let server = upstream(
            "/messages",
            ResponseTemplate::new(200).set_body_raw(ANTHROPIC_BODY, "application/json"),
        )
        .await;
        let hop = hop_on(WireProtocol::Anthropic, CredentialKind::Caller);

        let answer = relayed(
            exchange(&AnthropicOutbound, &hop, &server.uri(), false)
                .await
                .expect("served"),
        );

        assert_upstream_headers(&answer.headers);
        match answer.body {
            RelayedBody::Buffered { bytes, canonical } => {
                assert_eq!(bytes.as_ref(), ANTHROPIC_BODY.as_bytes());
                assert_eq!(canonical.usage.input_tokens, 7);
                assert_eq!(canonical.usage.output_tokens, 3);
            },
            RelayedBody::Streaming { .. } => panic!("buffered"),
        }
    }

    #[tokio::test]
    async fn the_same_answer_on_a_platform_hop_is_not_relayed() {
        let server = upstream(
            "/messages",
            ResponseTemplate::new(200).set_body_raw(ANTHROPIC_BODY, "application/json"),
        )
        .await;
        let hop = hop_on(WireProtocol::Anthropic, CredentialKind::Platform);

        let outcome = exchange(&AnthropicOutbound, &hop, &server.uri(), false)
            .await
            .expect("served");

        match outcome {
            OutboundOutcome::RawBuffered { body, .. } => assert_ne!(
                body.as_ref(),
                ANTHROPIC_BODY.as_bytes(),
                "the control: a governed hop corrects the contradictory stop_reason"
            ),
            _ => panic!("a governed raw-lane answer"),
        }
    }

    #[tokio::test]
    async fn an_openai_chat_answer_keeps_fields_the_canonical_model_does_not_carry() {
        let server = upstream(
            "/chat/completions",
            ResponseTemplate::new(200).set_body_raw(CHAT_BODY, "application/json"),
        )
        .await;
        let hop = hop_on(WireProtocol::OpenAiChat, CredentialKind::Caller);

        let answer = relayed(
            exchange(&OpenAiChatOutbound, &hop, &server.uri(), false)
                .await
                .expect("served"),
        );

        assert_upstream_headers(&answer.headers);
        match answer.body {
            RelayedBody::Buffered { bytes, canonical } => {
                assert_eq!(bytes.as_ref(), CHAT_BODY.as_bytes());
                assert_eq!(canonical.usage.input_tokens, 5);
            },
            RelayedBody::Streaming { .. } => panic!("buffered"),
        }
    }

    #[tokio::test]
    async fn a_stream_reaches_the_client_byte_for_byte_and_is_decoded_beside_it() {
        let server = upstream(
            "/chat/completions",
            ResponseTemplate::new(200).set_body_raw(CHAT_STREAM, "text/event-stream"),
        )
        .await;
        let hop = hop_on(WireProtocol::OpenAiChat, CredentialKind::Caller);

        let answer = relayed(
            exchange(&OpenAiChatOutbound, &hop, &server.uri(), true)
                .await
                .expect("served"),
        );

        assert_upstream_headers(&answer.headers);
        let RelayedBody::Streaming { mut bytes, events } = answer.body else {
            panic!("streaming");
        };
        let mut received = Vec::new();
        while let Some(chunk) = bytes.next().await {
            received.extend_from_slice(&chunk.expect("chunk"));
        }
        drop(bytes);
        assert_eq!(
            String::from_utf8(received).expect("utf8"),
            CHAT_STREAM,
            "the comment frame and the frame the decoder cannot read are both delivered"
        );
        let decoded: Vec<_> = events.collect().await;
        assert!(
            decoded.iter().any(|event| matches!(
                event,
                Ok(systemprompt_gateway::protocol::canonical::CanonicalEvent::MessageStop { .. })
            )),
            "the copy is decoded to its terminal event for the audit row"
        );
    }

    #[tokio::test]
    async fn an_upstream_error_is_handed_back_whole_with_its_headers() {
        let long = format!(
            "{{\"error\":{{\"message\":\"{}\"}}}}",
            "x".repeat(100 * 1024)
        );
        let server = upstream(
            "/messages",
            ResponseTemplate::new(429)
                .insert_header("retry-after", "17")
                .set_body_raw(long.clone(), "application/json"),
        )
        .await;
        let hop = hop_on(WireProtocol::Anthropic, CredentialKind::Caller);

        let refused = exchange(&AnthropicOutbound, &hop, &server.uri(), false).await;

        match refused {
            Err(OutboundError::Upstream(UpstreamError::Relayed {
                status,
                headers,
                body,
                ..
            })) => {
                assert_eq!(status, 429);
                assert_eq!(body.len(), long.len(), "past the 64 KiB summary cap, uncut");
                assert_eq!(
                    headers.get("retry-after").and_then(|v| v.to_str().ok()),
                    Some("17")
                );
                assert_upstream_headers(&headers);
            },
            _ => panic!("the upstream's own answer"),
        }
        assert_eq!(server.received_requests().await.expect("log").len(), 1);
    }

    #[test]
    fn returned_headers_withhold_framing_cookies_and_the_platform_namespace() {
        for kept in [
            "content-type",
            "retry-after",
            "request-id",
            "x-ratelimit-limit",
            "date",
        ] {
            assert!(is_returned_response_header(kept), "{kept}");
        }
        for withheld in [
            "Content-Length",
            "transfer-encoding",
            "connection",
            "set-cookie",
            "x-systemprompt-request-id",
            "X-Systemprompt-Memory-Ids",
        ] {
            assert!(!is_returned_response_header(withheld), "{withheld}");
        }
    }
}
