//! HTTP-delivery tests for WebhookService::send_webhook / test_endpoint.
//!
//! Target: crates/domain/agent/src/services/external_integrations/webhook/
//! service/delivery.rs
//!
//! Drives the real reqwest client against a wiremock server bound to loopback
//! (accepted by the outbound-URL guard) to exercise the success, non-2xx, and
//! transport-error branches, plus header and signature propagation.

use std::collections::HashMap;
use systemprompt_agent::models::external_integrations::WebhookEndpoint;
use systemprompt_agent::services::external_integrations::WebhookService;
use systemprompt_agent::services::external_integrations::webhook::WebhookConfig;
use systemprompt_identifiers::WebhookEndpointId;
use wiremock::matchers::{header, header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn send_webhook_success_2xx() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/hook"))
        .respond_with(ResponseTemplate::new(200).set_body_string("received"))
        .expect(1)
        .mount(&server)
        .await;

    let service = WebhookService::new();
    let url = format!("{}/hook", server.uri());
    let result = service
        .send_webhook(&url, serde_json::json!({"event": "ping"}), None)
        .await
        .expect("delivery result");

    assert!(result.success);
    assert_eq!(result.status_code, 200);
    assert_eq!(result.response_body, "received");
    assert!(result.error.is_none());
}

#[tokio::test]
async fn send_webhook_non_2xx_marks_failure() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let service = WebhookService::new();
    let result = service
        .send_webhook(&server.uri(), serde_json::json!({}), None)
        .await
        .expect("delivery result");

    assert!(!result.success);
    assert_eq!(result.status_code, 500);
    assert_eq!(result.response_body, "boom");
    assert!(result.error.is_none());
}

#[tokio::test]
async fn send_webhook_forwards_custom_header_and_signature() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(header("X-Custom", "yes"))
        .and(header_exists("X-Webhook-Signature"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let mut headers = HashMap::new();
    headers.insert("X-Custom".to_owned(), "yes".to_owned());
    let config = WebhookConfig {
        secret: Some("shh".to_owned()),
        headers,
        timeout: Some(std::time::Duration::from_secs(5)),
    };

    let service = WebhookService::new();
    let result = service
        .send_webhook(&server.uri(), serde_json::json!({"a": 1}), Some(config))
        .await
        .expect("delivery result");

    assert!(result.success);
    assert_eq!(result.status_code, 200);
}

#[tokio::test]
async fn send_webhook_transport_error_is_captured() {
    // Loopback port 1 passes the outbound-URL guard but refuses connections.
    let service = WebhookService::new();
    let result = service
        .send_webhook("http://127.0.0.1:1/hook", serde_json::json!({}), None)
        .await
        .expect("delivery result");

    assert!(!result.success);
    assert_eq!(result.status_code, 0);
    assert!(result.error.is_some());
}

#[tokio::test]
async fn send_webhook_rejects_non_loopback_http() {
    let service = WebhookService::new();
    let result = service
        .send_webhook("http://example.com/hook", serde_json::json!({}), None)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_endpoint_drives_delivery_against_server() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&server)
        .await;

    let service = WebhookService::new();
    let endpoint = WebhookEndpoint {
        id: WebhookEndpointId::new("wh-delivery-1"),
        url: server.uri(),
        events: vec!["push".to_owned()],
        secret: Some("topsecret".to_owned()),
        headers: HashMap::new(),
        active: true,
    };
    service
        .register_endpoint(endpoint)
        .await
        .expect("register endpoint");

    let result = service
        .test_endpoint(&WebhookEndpointId::new("wh-delivery-1"))
        .await
        .expect("test endpoint");

    assert!(result.success);
    assert_eq!(result.status_code, 200);
    assert_eq!(result.endpoint_id.as_str(), "wh-delivery-1");
}
