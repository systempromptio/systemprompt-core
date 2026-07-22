//! Behaviour tests for `run_checkout_callback_flow`: every callback-handler
//! branch (error, completed, unknown/missing status, missing IDs, pending
//! provisioning over SSE) plus the `/status/{tenant_id}` endpoint. The flow
//! binds a fixed callback port, so all scenarios run sequentially in single
//! tests rather than in parallel.

use std::time::Duration;

use serde_json::json;
use systemprompt_cloud::error::CloudError;
use systemprompt_cloud::{
    CheckoutCallbackResult, CheckoutTemplates, CloudApiClient, run_checkout_callback_flow,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEMPLATES: CheckoutTemplates = CheckoutTemplates {
    success_html: "<p>done {{TENANT_ID}}</p>",
    error_html: "<p>checkout-err</p>",
    waiting_html: "<p>waiting</p>",
};

const CALLBACK_BASE: &str = "http://127.0.0.1:8766";

fn spawn_flow(
    api_url: String,
) -> tokio::task::JoinHandle<Result<CheckoutCallbackResult, CloudError>> {
    tokio::spawn(async move {
        let client = CloudApiClient::new(&api_url, "op-token").expect("client");
        run_checkout_callback_flow(&client, "http://127.0.0.1:9/never-opened", TEMPLATES).await
    })
}

fn is_addr_in_use(err: &CloudError) -> bool {
    matches!(err, CloudError::Io(e) if e.kind() == std::io::ErrorKind::AddrInUse)
}

async fn hit(
    flow: &mut tokio::task::JoinHandle<Result<CheckoutCallbackResult, CloudError>>,
    path_and_query: &str,
) -> Option<String> {
    loop {
        if flow.is_finished() {
            return None;
        }
        match reqwest::get(format!("{CALLBACK_BASE}{path_and_query}")).await {
            Ok(response) => return Some(response.text().await.expect("callback body")),
            Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
        }
    }
}

async fn drive(
    api_url: &str,
    path_and_query: &str,
) -> (String, Result<CheckoutCallbackResult, CloudError>) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    loop {
        let mut flow = spawn_flow(api_url.to_owned());
        if let Some(body) = hit(&mut flow, path_and_query).await {
            return (body, flow.await.expect("join"));
        }
        match flow.await.expect("join") {
            Err(e) if is_addr_in_use(&e) => {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "callback port stayed in use"
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
            },
            other => panic!("flow ended before callback was delivered: {other:?}"),
        }
    }
}

fn sse_body(event_type: &str) -> String {
    let data = json!({
        "checkout_session_id": "cs-1",
        "tenant_id": "t-prov",
        "tenant_name": "Prov Tenant",
        "event_type": event_type,
        "status": "in_progress",
        "message": "step done",
        "app_url": "https://t-prov.app",
        "fly_app_name": "fly-t-prov"
    });
    format!("event: provisioning\ndata: {data}\n\n")
}

async fn mount_checkout_sse(server: &MockServer, body: String) {
    Mock::given(method("GET"))
        .and(path("/api/v1/checkout/cs-1/events"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(body.into_bytes(), "text/event-stream"),
        )
        .mount(server)
        .await;
}

#[tokio::test]
async fn checkout_callback_flow_end_to_end() {
    unsafe { std::env::set_var("BROWSER", "/bin/true") };
    terminal_branches().await;
    pending_provisioning_over_sse().await;
    status_endpoint_proxies_tenant_status().await;
}

async fn terminal_branches() {
    let server = MockServer::start().await;

    let (body, result) = drive(&server.uri(), "/callback?error=payment_declined").await;
    assert_eq!(body, "<p>checkout-err</p>");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("Checkout error: payment_declined"),
        "got {err}"
    );

    let (body, result) = drive(
        &server.uri(),
        "/callback?transaction_id=tx-1&tenant_id=t-1&status=completed",
    )
    .await;
    assert_eq!(body, "<p>done t-1</p>");
    let result = result.expect("completed checkout");
    assert_eq!(result.tenant_id.as_str(), "t-1");
    assert_eq!(result.transaction_id.as_str(), "tx-1");
    assert!(!result.needs_deploy);
    assert!(result.fly_app_name.is_none());

    let (body, result) = drive(
        &server.uri(),
        "/callback?transaction_id=tx-2&tenant_id=t-2&status=cancelled",
    )
    .await;
    assert_eq!(body, "<p>checkout-err</p>");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("Checkout status: cancelled"),
        "got {err}"
    );

    let (body, result) = drive(&server.uri(), "/callback?transaction_id=tx-3&tenant_id=t-3").await;
    assert_eq!(body, "<p>checkout-err</p>");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("missing required 'status'"),
        "got {err}"
    );

    let (body, result) = drive(&server.uri(), "/callback").await;
    assert_eq!(body, "<p>checkout-err</p>");
    let err = result.unwrap_err();
    assert!(
        err.to_string()
            .contains("Missing transaction_id or tenant_id"),
        "got {err}"
    );

    let (body, result) = drive(&server.uri(), "/callback?status=pending").await;
    assert_eq!(body, "<p>checkout-err</p>");
    let err = result.unwrap_err();
    assert!(
        err.to_string()
            .contains("Pending status but no checkout_session_id"),
        "got {err}"
    );
}

async fn pending_provisioning_over_sse() {
    let server = MockServer::start().await;
    mount_checkout_sse(&server, sse_body("infrastructure_ready")).await;
    let (body, result) = drive(
        &server.uri(),
        "/callback?status=pending&checkout_session_id=cs-1",
    )
    .await;
    assert_eq!(body, "<p>waiting</p>");
    let result = result.expect("infrastructure ready");
    assert!(result.needs_deploy);
    assert_eq!(result.tenant_id.as_str(), "t-prov");
    assert_eq!(result.fly_app_name.as_deref(), Some("fly-t-prov"));
    assert_eq!(result.transaction_id.as_str(), "cs-1");
    drop(server);

    let server = MockServer::start().await;
    mount_checkout_sse(&server, sse_body("tenant_ready")).await;
    let (body, result) = drive(
        &server.uri(),
        "/callback?status=pending&checkout_session_id=cs-1&transaction_id=tx-p",
    )
    .await;
    assert_eq!(body, "<p>waiting</p>");
    let result = result.expect("tenant ready");
    assert!(!result.needs_deploy);
    assert_eq!(result.transaction_id.as_str(), "tx-p");
    drop(server);

    let server = MockServer::start().await;
    mount_checkout_sse(&server, sse_body("provisioning_failed")).await;
    let (body, result) = drive(
        &server.uri(),
        "/callback?status=pending&checkout_session_id=cs-1",
    )
    .await;
    assert_eq!(body, "<p>waiting</p>");
    match result.unwrap_err() {
        CloudError::ProvisioningFailed { message } => assert_eq!(message, "step done"),
        other => panic!("expected ProvisioningFailed, got {other:?}"),
    }
    drop(server);

    let server = MockServer::start().await;
    mount_checkout_sse(&server, sse_body("vm_provisioned")).await;
    let (body, result) = drive(
        &server.uri(),
        "/callback?status=pending&checkout_session_id=cs-1",
    )
    .await;
    assert_eq!(body, "<p>waiting</p>");
    match result.unwrap_err() {
        CloudError::SseStream { message } => {
            assert!(message.contains("closed unexpectedly"), "got {message}");
        },
        other => panic!("expected SseStream, got {other:?}"),
    }
}

async fn status_endpoint_proxies_tenant_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "tenant_bearer",
            "expires_in": 600
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-live/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "status": "provisioning",
                "message": "almost there",
                "app_url": "https://t-live.app"
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-broken/status"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    let (mut flow, live_body) = loop {
        let mut flow = spawn_flow(server.uri());
        if let Some(body) = hit(&mut flow, "/status/t-live").await {
            break (flow, body);
        }
        match flow.await.expect("join") {
            Err(e) if is_addr_in_use(&e) => {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "callback port stayed in use"
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
            },
            other => panic!("flow ended before status query: {other:?}"),
        }
    };

    let live: serde_json::Value = serde_json::from_str(&live_body).expect("status json");
    assert_eq!(live["status"], "provisioning");
    assert_eq!(live["message"], "almost there");
    assert_eq!(live["app_url"], "https://t-live.app");

    let broken_body = hit(&mut flow, "/status/t-broken")
        .await
        .expect("flow alive");
    let broken: serde_json::Value = serde_json::from_str(&broken_body).expect("status json");
    assert_eq!(broken["status"], "error");
    assert!(broken["message"].as_str().is_some_and(|m| !m.is_empty()));
    assert!(broken["app_url"].is_null());

    let body = hit(&mut flow, "/callback?error=cancelled")
        .await
        .expect("flow alive");
    assert_eq!(body, "<p>checkout-err</p>");
    let err = flow.await.expect("join").unwrap_err();
    assert!(
        err.to_string().contains("Checkout error: cancelled"),
        "got {err}"
    );
}
