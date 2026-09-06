//! Bounded retry of transient upstream failures on the outbound send path.
//!
//! Every test drives the real `AnthropicOutbound` adapter against a wiremock
//! upstream, so it exercises the retry loop exactly where production does —
//! inside `send_checked` — rather than through a private hook. The zero-delay
//! policy is scoped per test so a four-attempt budget costs milliseconds.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::json;
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalToolChoice, Role,
};
use systemprompt_api::services::gateway::protocol::outbound::anthropic::AnthropicOutbound;
use systemprompt_api::services::gateway::protocol::outbound::retry::{
    MAX_ATTEMPTS, RetryPolicy, backoff_delay, effective_delay, is_retryable, observing_retries,
    parse_retry_after, with_policy,
};
use systemprompt_api::services::gateway::protocol::outbound::{
    OutboundAdapter, OutboundCtx, OutboundOutcome, UpstreamError,
};
use systemprompt_identifiers::{ProviderId, RouteId};
use systemprompt_models::services::GatewayRoute;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn route() -> GatewayRoute {
    GatewayRoute {
        id: RouteId::new("r1"),
        model_pattern: "*".into(),
        provider: ProviderId::new("anthropic"),
        upstream_model: Some("upstream-1".into()),
        extra_headers: HashMap::new(),
        pricing: None,
        when: None,
        requires: None,
    }
}

fn request() -> CanonicalRequest {
    CanonicalRequest {
        model: "m".into(),
        system: None,
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::Text("hi".into())],
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

fn ok_body() -> serde_json::Value {
    json!({
        "id": "msg_ok",
        "type": "message",
        "role": "assistant",
        "model": "upstream-1",
        "content": [{ "type": "text", "text": "recovered" }],
        "stop_reason": "end_turn",
        "usage": { "input_tokens": 1, "output_tokens": 2 }
    })
}

// Why: the adapter owns both halves of a send, and the retry sits between
// them, so the tests must go through the pair rather than either alone.
async fn send_once(endpoint: &str) -> anyhow::Result<OutboundOutcome> {
    let adapter = AnthropicOutbound;
    let route = route();
    let req = request();
    let ctx = OutboundCtx {
        route: &route,
        endpoint,
        api_key: "k",
        api_key_is_bearer: false,
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        forward_headers: &[],
        raw_body: None,
    };
    let body = adapter.build_body(&ctx)?;
    adapter.send(ctx, &body).await
}

async fn send_counting(endpoint: &str) -> (anyhow::Result<OutboundOutcome>, u32) {
    with_policy(
        RetryPolicy::immediate(),
        observing_retries(send_once(endpoint)),
    )
    .await
}

async fn mount_transient_then_ok(server: &MockServer, status: u16) {
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(status).set_body_string("overloaded"))
        .up_to_n_times(1)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_body()))
        .mount(server)
        .await;
}

#[tokio::test]
async fn retries_once_after_upstream_429() {
    let server = MockServer::start().await;
    mount_transient_then_ok(&server, 429).await;

    let (outcome, retries) = send_counting(&server.uri()).await;

    assert!(matches!(
        outcome.expect("recovered"),
        OutboundOutcome::Buffered(_)
    ));
    assert_eq!(retries, 1);
    assert_eq!(server.received_requests().await.expect("log").len(), 2);
}

#[tokio::test]
async fn retries_once_after_upstream_503() {
    let server = MockServer::start().await;
    mount_transient_then_ok(&server, 503).await;

    let (outcome, retries) = send_counting(&server.uri()).await;

    assert!(matches!(
        outcome.expect("recovered"),
        OutboundOutcome::Buffered(_)
    ));
    assert_eq!(retries, 1);
    assert_eq!(server.received_requests().await.expect("log").len(), 2);
}

async fn assert_not_retried(status: u16) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(status).set_body_string("nope"))
        .mount(&server)
        .await;

    let (outcome, retries) = send_counting(&server.uri()).await;

    let err = outcome.err().expect("upstream error");
    let upstream = err.downcast_ref::<UpstreamError>().expect("upstream kind");
    match upstream {
        UpstreamError::Status { status: got, .. } => assert_eq!(*got, status),
        UpstreamError::Transport { .. } => panic!("expected a status error"),
    }
    assert_eq!(retries, 0);
    assert_eq!(server.received_requests().await.expect("log").len(), 1);
}

#[tokio::test]
async fn does_not_retry_bad_request() {
    assert_not_retried(400).await;
}

#[tokio::test]
async fn does_not_retry_unauthorized() {
    assert_not_retried(401).await;
}

#[tokio::test]
async fn does_not_retry_server_error() {
    assert_not_retried(500).await;
}

#[tokio::test]
async fn exhausted_budget_relays_the_final_429_verbatim() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "7")
                .set_body_string(r#"{"error":{"message":"capacity"}}"#),
        )
        .mount(&server)
        .await;

    let (outcome, retries) = send_counting(&server.uri()).await;

    let err = outcome.err().expect("upstream error");
    let upstream = err.downcast_ref::<UpstreamError>().expect("upstream kind");
    match upstream {
        UpstreamError::Status {
            status,
            body,
            retry_after,
            message,
            ..
        } => {
            assert_eq!(*status, 429);
            assert_eq!(body.as_ref(), br#"{"error":{"message":"capacity"}}"#);
            assert_eq!(retry_after.as_deref(), Some("7"));
            assert_eq!(message, "capacity");
        },
        UpstreamError::Transport { .. } => panic!("expected a status error"),
    }
    assert_eq!(retries, MAX_ATTEMPTS - 1);
    assert_eq!(
        server.received_requests().await.expect("log").len(),
        MAX_ATTEMPTS as usize
    );
}

#[tokio::test]
async fn only_capacity_statuses_are_retryable() {
    assert!(is_retryable(429));
    assert!(is_retryable(503));
    for status in [400, 401, 404, 409, 500, 502, 504] {
        assert!(!is_retryable(status), "{status} must not be retried");
    }
}

#[tokio::test]
async fn backoff_doubles_then_clamps_to_the_ceiling() {
    let policy = RetryPolicy {
        jitter_ratio: 0.0,
        ..RetryPolicy::default()
    };
    assert_eq!(backoff_delay(1, &policy), Duration::from_secs(1));
    assert_eq!(backoff_delay(2, &policy), Duration::from_secs(2));
    assert_eq!(backoff_delay(3, &policy), Duration::from_secs(4));
    assert_eq!(backoff_delay(9, &policy), Duration::from_secs(30));
    assert_eq!(backoff_delay(64, &policy), Duration::from_secs(30));
}

#[tokio::test]
async fn jitter_stays_within_a_quarter_of_the_curve() {
    let policy = RetryPolicy::default();
    for _ in 0..64 {
        let delay = backoff_delay(1, &policy).as_millis();
        assert!((750..=1250).contains(&delay), "{delay} out of jitter band");
    }
}

#[tokio::test]
async fn retry_after_is_obeyed_only_when_it_is_longer() {
    let policy = RetryPolicy {
        jitter_ratio: 0.0,
        ..RetryPolicy::default()
    };
    let now = chrono::Utc::now();
    assert_eq!(
        effective_delay(1, Some("5"), &policy, now),
        Duration::from_secs(5)
    );
    assert_eq!(
        effective_delay(3, Some("1"), &policy, now),
        Duration::from_secs(4)
    );
    assert_eq!(
        effective_delay(1, Some("600"), &policy, now),
        Duration::from_secs(30)
    );
    assert_eq!(
        effective_delay(1, Some("nonsense"), &policy, now),
        Duration::from_secs(1)
    );
}

#[tokio::test]
async fn retry_after_accepts_an_http_date() {
    let now = chrono::Utc::now();
    let at = now + chrono::Duration::seconds(12);
    let header = at.to_rfc2822();
    let parsed = parse_retry_after(&header, now).expect("date parses");
    assert!(
        (10..=13).contains(&parsed.as_secs()),
        "{parsed:?} not near 12s"
    );
    let past = (now - chrono::Duration::seconds(60)).to_rfc2822();
    assert_eq!(parse_retry_after(&past, now), Some(Duration::ZERO));
}

#[tokio::test]
async fn a_disabled_policy_makes_exactly_one_attempt() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(503).set_body_string("down"))
        .mount(&server)
        .await;

    let (outcome, retries) = with_policy(
        RetryPolicy::none(),
        observing_retries(send_once(&server.uri())),
    )
    .await;

    assert!(outcome.is_err());
    assert_eq!(retries, 0);
    assert_eq!(server.received_requests().await.expect("log").len(), 1);
}
