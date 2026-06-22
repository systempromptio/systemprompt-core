//! Unit tests for the SSE subscription streams on `CloudApiClient`,
//! driven against a wiremock server that serves a `text/event-stream`
//! body. The wiremock mocks deliberately do NOT match on the `Accept`
//! header: `reqwest-eventsource` negotiates that header itself and a
//! matcher on it makes the stub silently miss.

use futures::StreamExt;
use systemprompt_cloud::CloudApiClient;
use systemprompt_identifiers::{CheckoutSessionId, TenantId};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sse_response(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(body.to_owned().into_bytes(), "text/event-stream")
}

#[tokio::test]
async fn provisioning_stream_yields_parsed_ready_event() {
    let server = MockServer::start().await;
    let body = concat!(
        "event: provisioning\n",
        "data: {\"tenant_id\":\"t-1\",\"event_type\":\"tenant_ready\",\"status\":\"ready\"}\n\n"
    );
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-1/events"))
        .respond_with(sse_response(body))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let mut stream = client.subscribe_provisioning_events(&TenantId::new("t-1"));

    let first = stream.next().await.expect("one event").expect("ok event");
    assert_eq!(first.tenant_id.as_str(), "t-1");
    assert!(matches!(
        first.event_type,
        systemprompt_cloud::api_client::ProvisioningEventType::TenantReady
    ));
    assert_eq!(first.status, "ready");
}

#[tokio::test]
async fn provisioning_stream_skips_unparseable_data_then_ends() {
    let server = MockServer::start().await;
    let body = concat!(
        "event: provisioning\n",
        "data: not-json\n\n",
        "event: heartbeat\n",
        "data: {}\n\n"
    );
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-2/events"))
        .respond_with(sse_response(body))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let mut stream = client.subscribe_provisioning_events(&TenantId::new("t-2"));

    // The unparseable frame is logged and dropped, the heartbeat is ignored,
    // and the stream then ends without yielding an item.
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn provisioning_stream_message_event_name_is_accepted() {
    let server = MockServer::start().await;
    let body = concat!(
        "event: message\n",
        "data: {\"tenant_id\":\"t-3\",\"event_type\":\"vm_provisioned\",\"status\":\"working\"}\n\n"
    );
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-3/events"))
        .respond_with(sse_response(body))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let mut stream = client.subscribe_provisioning_events(&TenantId::new("t-3"));

    let event = stream.next().await.expect("event").expect("ok");
    assert!(matches!(
        event.event_type,
        systemprompt_cloud::api_client::ProvisioningEventType::VmProvisioned
    ));
}

#[tokio::test]
async fn checkout_stream_yields_parsed_event() {
    let server = MockServer::start().await;
    let body = concat!(
        "event: provisioning\n",
        "data: {\"checkout_session_id\":\"cs-1\",\"tenant_id\":\"t-9\",\"tenant_name\":\"acme\",",
        "\"event_type\":\"infrastructure_ready\",\"status\":\"provisioning\",",
        "\"fly_app_name\":\"acme-app\"}\n\n"
    );
    Mock::given(method("GET"))
        .and(path("/api/v1/checkout/cs-1/events"))
        .respond_with(sse_response(body))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let mut stream = client.subscribe_checkout_events(&CheckoutSessionId::new("cs-1"));

    let event = stream.next().await.expect("event").expect("ok");
    assert_eq!(event.tenant_id.as_str(), "t-9");
    assert_eq!(event.tenant_name, "acme");
    assert_eq!(event.fly_app_name.as_deref(), Some("acme-app"));
    assert!(matches!(
        event.event_type,
        systemprompt_cloud::api_client::ProvisioningEventType::InfrastructureReady
    ));
}

#[tokio::test]
async fn checkout_stream_ignores_non_provisioning_event_names() {
    let server = MockServer::start().await;
    let body = concat!("event: heartbeat\n", "data: {}\n\n");
    Mock::given(method("GET"))
        .and(path("/api/v1/checkout/cs-2/events"))
        .respond_with(sse_response(body))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let mut stream = client.subscribe_checkout_events(&CheckoutSessionId::new("cs-2"));

    assert!(stream.next().await.is_none());
}
