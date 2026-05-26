//! Unit tests for WebhookService in-memory endpoint registry.
//!
//! Targets:
//! - crates/domain/agent/src/services/external_integrations/webhook/service/mod.rs

use std::collections::HashMap;
use systemprompt_agent::services::external_integrations::WebhookService;
use systemprompt_identifiers::WebhookEndpointId;
use systemprompt_agent::models::external_integrations::{
    WebhookEndpoint, WebhookRequest,
};

fn make_endpoint(id: &str, secret: Option<&str>, events: Vec<&str>, active: bool) -> WebhookEndpoint {
    WebhookEndpoint {
        id: WebhookEndpointId::new(id),
        url: "https://example.com/hook".to_string(),
        events: events.into_iter().map(String::from).collect(),
        secret: secret.map(String::from),
        headers: HashMap::new(),
        active,
    }
}

fn make_request(headers: &[(&str, &str)], body: serde_json::Value, signature: Option<&str>) -> WebhookRequest {
    WebhookRequest {
        headers: headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        body,
        signature: signature.map(String::from),
    }
}

#[tokio::test]
async fn register_and_get_endpoint() {
    let service = WebhookService::new();
    let endpoint = make_endpoint("ep-1", None, vec!["push"], true);
    let id = service.register_endpoint(endpoint.clone()).await.unwrap();
    assert_eq!(id.as_str(), "ep-1");

    let fetched = service.get_endpoint(&id).await.unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().url, "https://example.com/hook");
}

#[tokio::test]
async fn register_endpoint_generates_id_when_empty() {
    let service = WebhookService::new();
    let endpoint = make_endpoint("", None, vec![], true);
    let id = service.register_endpoint(endpoint).await.unwrap();
    assert!(!id.as_str().is_empty());
}

#[tokio::test]
async fn get_endpoint_missing_returns_none() {
    let service = WebhookService::new();
    let id = WebhookEndpointId::new("does-not-exist");
    assert!(service.get_endpoint(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn list_endpoints_after_register() {
    let service = WebhookService::new();
    service
        .register_endpoint(make_endpoint("a", None, vec![], true))
        .await
        .unwrap();
    service
        .register_endpoint(make_endpoint("b", None, vec![], true))
        .await
        .unwrap();

    let all = service.list_endpoints().await.unwrap();
    assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn update_endpoint_replaces() {
    let service = WebhookService::new();
    let mut endpoint = make_endpoint("up-1", None, vec![], true);
    service.register_endpoint(endpoint.clone()).await.unwrap();

    endpoint.url = "https://new.example/hook".to_string();
    service.update_endpoint(endpoint).await.unwrap();

    let fetched = service
        .get_endpoint(&WebhookEndpointId::new("up-1"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.url, "https://new.example/hook");
}

#[tokio::test]
async fn remove_endpoint_returns_true_when_present() {
    let service = WebhookService::new();
    let endpoint = make_endpoint("rm-1", None, vec![], true);
    service.register_endpoint(endpoint).await.unwrap();

    let removed = service
        .remove_endpoint(&WebhookEndpointId::new("rm-1"))
        .await
        .unwrap();
    assert!(removed);

    let removed_again = service
        .remove_endpoint(&WebhookEndpointId::new("rm-1"))
        .await
        .unwrap();
    assert!(!removed_again);
}

#[tokio::test]
async fn handle_webhook_endpoint_not_found_returns_err() {
    let service = WebhookService::new();
    let req = make_request(&[], serde_json::json!({}), None);
    let result = service
        .handle_webhook(&WebhookEndpointId::new("missing"), req)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn handle_webhook_inactive_endpoint_returns_404() {
    let service = WebhookService::new();
    service
        .register_endpoint(make_endpoint("inactive", None, vec![], false))
        .await
        .unwrap();

    let req = make_request(&[("x-webhook-event", "push")], serde_json::json!({}), None);
    let response = service
        .handle_webhook(&WebhookEndpointId::new("inactive"), req)
        .await
        .unwrap();
    assert_eq!(response.status, 404);
}

#[tokio::test]
async fn handle_webhook_event_not_subscribed_returns_200() {
    let service = WebhookService::new();
    service
        .register_endpoint(make_endpoint("ep", None, vec!["push"], true))
        .await
        .unwrap();

    let req = make_request(
        &[("x-webhook-event", "delete")],
        serde_json::json!({"action": "delete"}),
        None,
    );
    let response = service
        .handle_webhook(&WebhookEndpointId::new("ep"), req)
        .await
        .unwrap();
    assert_eq!(response.status, 200);
    let body = response.body.unwrap();
    assert!(body["message"].as_str().unwrap().contains("not subscribed"));
}

#[tokio::test]
async fn handle_webhook_wildcard_event_subscription() {
    let service = WebhookService::new();
    service
        .register_endpoint(make_endpoint("ep", None, vec!["*"], true))
        .await
        .unwrap();

    let req = make_request(
        &[("x-event-type", "anything")],
        serde_json::json!({}),
        None,
    );
    let response = service
        .handle_webhook(&WebhookEndpointId::new("ep"), req)
        .await
        .unwrap();
    assert_eq!(response.status, 200);
    let body = response.body.unwrap();
    assert!(body["message"].as_str().unwrap().contains("successfully"));
    assert_eq!(body["event_type"], "anything");
}

#[tokio::test]
async fn handle_webhook_event_type_default_unknown() {
    let service = WebhookService::new();
    service
        .register_endpoint(make_endpoint("ep", None, vec![], true))
        .await
        .unwrap();

    let req = make_request(&[], serde_json::json!({}), None);
    let response = service
        .handle_webhook(&WebhookEndpointId::new("ep"), req)
        .await
        .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.body.unwrap()["event_type"], "unknown");
}

#[tokio::test]
async fn handle_webhook_github_event_header() {
    let service = WebhookService::new();
    service
        .register_endpoint(make_endpoint("ep", None, vec![], true))
        .await
        .unwrap();

    let req = make_request(
        &[("x-github-event", "pull_request")],
        serde_json::json!({}),
        None,
    );
    let response = service
        .handle_webhook(&WebhookEndpointId::new("ep"), req)
        .await
        .unwrap();
    assert_eq!(response.body.unwrap()["event_type"], "pull_request");
}

#[tokio::test]
async fn verify_signature_endpoint_missing() {
    let service = WebhookService::new();
    let body = serde_json::json!({"hi": 1});
    let result = service
        .verify_signature(&WebhookEndpointId::new("nope"), &body, "sha256=abc")
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn verify_signature_no_secret_configured() {
    let service = WebhookService::new();
    service
        .register_endpoint(make_endpoint("nosec", None, vec![], true))
        .await
        .unwrap();

    let body = serde_json::json!({"foo": "bar"});
    let result = service
        .verify_signature(&WebhookEndpointId::new("nosec"), &body, "sha256=abc")
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn handle_webhook_signature_mismatch_returns_401() {
    let service = WebhookService::new();
    service
        .register_endpoint(make_endpoint("sec", Some("supersecret"), vec![], true))
        .await
        .unwrap();

    let req = make_request(
        &[("x-webhook-event", "ping")],
        serde_json::json!({"data": 1}),
        Some("sha256=deadbeef"),
    );
    let response = service
        .handle_webhook(&WebhookEndpointId::new("sec"), req)
        .await
        .unwrap();
    assert_eq!(response.status, 401);
}

#[tokio::test]
async fn webhook_service_default_creates_empty() {
    let service = WebhookService::default();
    let all = service.list_endpoints().await.unwrap();
    assert!(all.is_empty());
}

#[tokio::test]
async fn get_endpoint_stats_unknown_returns_err() {
    let service = WebhookService::new();
    let result = service
        .get_endpoint_stats(&WebhookEndpointId::new("nope"))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_endpoint_stats_returns_zeroed_for_known() {
    let service = WebhookService::new();
    service
        .register_endpoint(make_endpoint("stat", None, vec![], true))
        .await
        .unwrap();

    let stats = service
        .get_endpoint_stats(&WebhookEndpointId::new("stat"))
        .await
        .unwrap();
    assert_eq!(stats.endpoint_id.as_str(), "stat");
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.successful_requests, 0);
    assert_eq!(stats.failed_requests, 0);
    assert!(stats.last_request_at.is_none());
}

#[tokio::test]
async fn send_webhook_rejects_invalid_url() {
    let service = WebhookService::new();
    let result = service
        .send_webhook("not-a-url", serde_json::json!({}), None)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_endpoint_unknown_returns_err() {
    let service = WebhookService::new();
    let result = service
        .test_endpoint(&WebhookEndpointId::new("ghost"))
        .await;
    assert!(result.is_err());
}
