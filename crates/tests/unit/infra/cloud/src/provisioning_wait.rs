//! Unit tests for the public `wait_for_provisioning` watcher. It consumes
//! the provisioning SSE stream and, on stream error or unexpected close,
//! falls back to polling `GET /status`. Tests drive both the SSE happy
//! paths and the polling fallback against a wiremock server.
//!
//! The SSE mocks deliberately do NOT match on the `Accept` header
//! (`reqwest-eventsource` sets it itself).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;
use systemprompt_cloud::{CloudApiClient, CloudError, wait_for_provisioning};
use systemprompt_identifiers::TenantId;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sse_response(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(body.to_owned().into_bytes(), "text/event-stream")
}

/// The polling fallback hits `get_tenant_status`, which first acquires a
/// tenant access token via RFC 8693 token-exchange. Stub it so the poll
/// resolves on the first attempt rather than erroring and retrying.
async fn token_mock(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "tenant_bearer",
            "expires_in": 600
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn returns_event_when_sse_reports_tenant_ready() {
    let server = MockServer::start().await;
    let body = concat!(
        "event: provisioning\n",
        "data: {\"tenant_id\":\"t-ready\",\"event_type\":\"tenant_ready\",\"status\":\"ready\",",
        "\"app_url\":\"https://app.test\"}\n\n"
    );
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-ready/events"))
        .respond_with(sse_response(body))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let seen = Arc::new(AtomicUsize::new(0));
    let seen_cl = Arc::clone(&seen);
    let event = wait_for_provisioning(&client, &TenantId::new("t-ready"), move |_e| {
        seen_cl.fetch_add(1, Ordering::SeqCst);
    })
    .await
    .expect("ready");

    assert_eq!(event.tenant_id.as_str(), "t-ready");
    assert_eq!(event.app_url.as_deref(), Some("https://app.test"));
    assert!(seen.load(Ordering::SeqCst) >= 1, "callback must fire");
}

#[tokio::test]
async fn returns_failed_error_when_sse_reports_provisioning_failed() {
    let server = MockServer::start().await;
    let body = concat!(
        "event: provisioning\n",
        "data: {\"tenant_id\":\"t-fail\",\"event_type\":\"provisioning_failed\",",
        "\"status\":\"failed\",\"message\":\"disk full\"}\n\n"
    );
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-fail/events"))
        .respond_with(sse_response(body))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let err = wait_for_provisioning(&client, &TenantId::new("t-fail"), |_e| {})
        .await
        .expect_err("must fail");

    match err {
        CloudError::ProvisioningFailed { message } => assert!(message.contains("disk full")),
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[tokio::test]
async fn falls_back_to_polling_when_sse_stream_closes_without_terminal_event() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    // SSE delivers a non-terminal event then closes; the watcher must fall
    // back to polling the status endpoint, which immediately reports ready.
    let body = concat!(
        "event: provisioning\n",
        "data: {\"tenant_id\":\"t-poll\",\"event_type\":\"vm_provisioning_started\",",
        "\"status\":\"working\"}\n\n"
    );
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-poll/events"))
        .respond_with(sse_response(body))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-poll/status"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            br#"{"data":{"tenant_id":"t-poll","status":"ready","app_url":"https://ready.test"}}"#
                .to_vec(),
            "application/json",
        ))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let event = wait_for_provisioning(&client, &TenantId::new("t-poll"), |_e| {})
        .await
        .expect("ready via polling");

    assert_eq!(event.status, "ready");
    assert_eq!(event.app_url.as_deref(), Some("https://ready.test"));
}

#[tokio::test]
async fn polling_fallback_reports_failed_status_as_error() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    // No SSE mock for this tenant: EventSource gets a 404, errors, and the
    // watcher falls straight through to polling, which reports failure.
    let polls = Arc::new(AtomicUsize::new(0));
    let polls_cl = Arc::clone(&polls);
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-pollfail/status"))
        .respond_with(move |_req: &wiremock::Request| {
            polls_cl.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(200).set_body_raw(
                br#"{"data":{"tenant_id":"t-pollfail","status":"failed","message":"boom"}}"#
                    .to_vec(),
                "application/json",
            )
        })
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let err = wait_for_provisioning(&client, &TenantId::new("t-pollfail"), |_e| {})
        .await
        .expect_err("must fail");

    match err {
        CloudError::ProvisioningFailed { message } => assert!(message.contains("boom")),
        other => panic!("unexpected variant: {other:?}"),
    }
    assert!(polls.load(Ordering::SeqCst) >= 1);
}
