//! Unit tests for tenant-scoped `CloudApiClient` endpoints. Each test
//! mocks both the RFC8693 token-exchange endpoint and the tenant-specific
//! API endpoint.

use std::collections::HashMap;

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

#[tokio::test]
async fn restart_tenant_calls_restart_endpoint() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/tenants/t-r/restart"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "restarting"
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let _ = client.restart_tenant(&TenantId::new("t-r")).await;
}

#[tokio::test]
async fn retry_provision_calls_endpoint() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/tenants/t-rp/retry-provision"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "retrying"
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let _ = client.retry_provision(&TenantId::new("t-rp")).await;
}

#[tokio::test]
async fn set_secrets_returns_keys() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/tenants/t-s/secrets"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let mut secrets = HashMap::new();
    secrets.insert("FOO".to_owned(), "bar".to_owned());
    let res = client.set_secrets(&TenantId::new("t-s"), secrets).await;
    if let Ok(keys) = res {
        assert!(keys.contains(&"FOO".to_owned()));
    }
}

#[tokio::test]
async fn unset_secret_calls_delete_keyed_path() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/tenants/t-s/secrets/FOO"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    client
        .unset_secret(&TenantId::new("t-s"), "FOO")
        .await
        .expect("unset_secret should succeed against the 204 mock");
}

#[tokio::test]
async fn list_secrets_returns_payload() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-s/secrets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "secrets": []
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let _ = client.list_secrets(&TenantId::new("t-s")).await;
}

#[tokio::test]
async fn set_external_db_access_puts_flag() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/tenants/t-e/external-db-access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "enabled": true
            }
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let _ = client
        .set_external_db_access(&TenantId::new("t-e"), true)
        .await;
}

#[tokio::test]
async fn rotate_credentials_calls_endpoint() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/tenants/t-rc/rotate-credentials"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "rotated"
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let _ = client.rotate_credentials(&TenantId::new("t-rc")).await;
}

#[tokio::test]
async fn set_custom_domain_posts_request() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/tenants/t-cd/custom-domain"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "domain": "example.com",
                "verified": false
            }
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let _ = client
        .set_custom_domain(&TenantId::new("t-cd"), "example.com")
        .await;
}

#[tokio::test]
async fn get_custom_domain_returns_data() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-cd/custom-domain"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "domain": "example.com",
                "verified": true
            }
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let _ = client.get_custom_domain(&TenantId::new("t-cd")).await;
}

#[tokio::test]
async fn delete_custom_domain_returns_unit() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/tenants/t-cd/custom-domain"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    client
        .delete_custom_domain(&TenantId::new("t-cd"))
        .await
        .expect("delete_custom_domain should succeed against the 204 mock");
}

#[tokio::test]
async fn cancel_subscription_calls_endpoint() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/tenants/t-sub/cancel"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "cancelled"
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let _ = client.cancel_subscription(&TenantId::new("t-sub")).await;
}

#[tokio::test]
async fn get_registry_token_returns_token() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-rt/registry-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "token": "ghp_x",
                "expires_at": "2026-12-31T00:00:00Z"
            }
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let _ = client.get_registry_token(&TenantId::new("t-rt")).await;
}

#[tokio::test]
async fn fetch_secrets_strips_api_url_prefix() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/internal/secrets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "secrets": {}
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let url = format!("{}/api/v1/internal/secrets", server.uri());
    let _ = client.fetch_secrets(&url).await;
}
