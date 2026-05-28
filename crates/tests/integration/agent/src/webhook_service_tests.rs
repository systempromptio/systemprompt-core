use anyhow::Result;
use std::collections::HashMap;
use systemprompt_agent::models::external_integrations::{WebhookEndpoint, WebhookRequest};
use systemprompt_agent::services::external_integrations::webhook::WebhookService;
use systemprompt_identifiers::WebhookEndpointId;

fn endpoint(active: bool, secret: Option<&str>, events: &[&str]) -> WebhookEndpoint {
    WebhookEndpoint {
        id: WebhookEndpointId::generate(),
        url: "https://example.invalid/hook".into(),
        events: events.iter().map(|s| (*s).to_owned()).collect(),
        secret: secret.map(str::to_owned),
        headers: HashMap::new(),
        active,
    }
}

#[tokio::test]
async fn register_get_list_remove_endpoint() -> Result<()> {
    let svc = WebhookService::new();
    let ep = endpoint(true, None, &["push"]);
    let id = svc.register_endpoint(ep.clone()).await?;
    let got = svc.get_endpoint(&id).await?;
    assert!(got.is_some());

    let listed = svc.list_endpoints().await?;
    assert_eq!(listed.len(), 1);

    let updated = WebhookEndpoint {
        active: false,
        ..ep.clone()
    };
    svc.update_endpoint(updated).await?;
    let got2 = svc.get_endpoint(&id).await?.expect("present");
    assert!(!got2.active);

    let removed = svc.remove_endpoint(&id).await?;
    assert!(removed);
    let not_removed = svc.remove_endpoint(&id).await?;
    assert!(!not_removed);
    Ok(())
}

#[tokio::test]
async fn handle_webhook_inactive_endpoint_returns_404() -> Result<()> {
    let svc = WebhookService::new();
    let id = svc
        .register_endpoint(endpoint(false, None, &["push"]))
        .await?;
    let req = WebhookRequest {
        headers: HashMap::new(),
        body: serde_json::json!({"x": 1}),
        signature: None,
    };
    let resp = svc.handle_webhook(&id, req).await?;
    assert_eq!(resp.status, 404);
    Ok(())
}

#[tokio::test]
async fn handle_webhook_unsubscribed_event_is_acknowledged() -> Result<()> {
    let svc = WebhookService::new();
    let id = svc
        .register_endpoint(endpoint(true, None, &["push"]))
        .await?;
    let mut headers = HashMap::new();
    headers.insert("x-webhook-event".into(), "issue".into());
    let req = WebhookRequest {
        headers,
        body: serde_json::json!({}),
        signature: None,
    };
    let resp = svc.handle_webhook(&id, req).await?;
    assert_eq!(resp.status, 200);
    Ok(())
}

#[tokio::test]
async fn handle_webhook_wildcard_subscription_passes() -> Result<()> {
    let svc = WebhookService::new();
    let id = svc.register_endpoint(endpoint(true, None, &["*"])).await?;
    let req = WebhookRequest {
        headers: HashMap::new(),
        body: serde_json::json!({}),
        signature: None,
    };
    let resp = svc.handle_webhook(&id, req).await?;
    assert_eq!(resp.status, 200);
    Ok(())
}

#[tokio::test]
async fn handle_webhook_missing_endpoint_errors() -> Result<()> {
    let svc = WebhookService::new();
    let id = WebhookEndpointId::generate();
    let req = WebhookRequest {
        headers: HashMap::new(),
        body: serde_json::json!({}),
        signature: None,
    };
    let res = svc.handle_webhook(&id, req).await;
    assert!(res.is_err());
    Ok(())
}

#[tokio::test]
async fn handle_webhook_invalid_signature_returns_401() -> Result<()> {
    let svc = WebhookService::new();
    let id = svc
        .register_endpoint(endpoint(true, Some("sekret"), &["*"]))
        .await?;
    let req = WebhookRequest {
        headers: HashMap::new(),
        body: serde_json::json!({"x": 1}),
        signature: Some("not-the-right-sig".into()),
    };
    let resp = svc.handle_webhook(&id, req).await?;
    assert_eq!(resp.status, 401);
    Ok(())
}
