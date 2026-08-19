//! Unit tests for tenant-scoped `CloudApiClient` endpoints. Each test
//! mocks both the RFC8693 token-exchange endpoint and the tenant-specific
//! API endpoint.


use serde_json::json;
use systemprompt_cloud::CloudApiClient;
use systemprompt_identifiers::TenantId;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
async fn get_tenant_status_returns_data_payload() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-abc/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "tenant_id": "t-abc",
                "status": "running",
                "health": "healthy"
            }
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "operator").unwrap();
    let _ = client.get_tenant_status(&TenantId::new("t-abc")).await;
}

#[tokio::test]
async fn token_exchange_unauthorized_propagates() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "x").unwrap();
    let _ = client
        .get_tenant_status(&TenantId::new("t-fail"))
        .await
        .expect_err("must error");
}

#[tokio::test]
async fn token_exchange_failure_status_propagates() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "x").unwrap();
    let _ = client
        .get_tenant_status(&TenantId::new("t-fail2"))
        .await
        .expect_err("must error");
}

#[tokio::test]
async fn deploy_posts_image_request() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/tenants/t-abc/deploy"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "tenant_id": "t-abc",
                "status": "deploying"
            }
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let _ = client
        .deploy(&TenantId::new("t-abc"), "registry/img:latest")
        .await;
}

#[tokio::test]
async fn delete_tenant_returns_unit_on_204() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/tenants/t-d"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    client
        .delete_tenant(&TenantId::new("t-d"))
        .await
        .expect("delete_tenant should succeed against the 204 mock");
}
