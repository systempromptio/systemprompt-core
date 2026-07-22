//! Unit tests for the RFC 8693 token-exchange + single-retry-on-401 path
//! in `CloudApiClient`'s tenant-scoped HTTP helpers. A tenant endpoint that
//! returns 401 once must trigger a token-cache invalidation and exactly one
//! retry before the call succeeds.

use serde_json::json;
use systemprompt_cloud::CloudApiClient;
use systemprompt_cloud::error::CloudError;
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
async fn tenant_get_retries_once_after_401_then_succeeds() {
    let server = MockServer::start().await;
    token_mock(&server).await;

    // First match: a single 401 (forces cache invalidation + retry).
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-rt/status"))
        .respond_with(ResponseTemplate::new(401))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;

    // Fallback match: subsequent calls succeed.
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-rt/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "tenant_id": "t-rt", "status": "running" }
        })))
        .with_priority(2)
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let status = client
        .get_tenant_status(&TenantId::new("t-rt"))
        .await
        .expect("retry should succeed");
    assert_eq!(status.status, "running");
}

#[tokio::test]
async fn tenant_get_persistent_401_surfaces_unauthorized() {
    let server = MockServer::start().await;
    token_mock(&server).await;

    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-401/status"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let err = client
        .get_tenant_status(&TenantId::new("t-401"))
        .await
        .expect_err("must error after retry");
    assert!(matches!(err, CloudError::Unauthorized));
}

#[tokio::test]
async fn tenant_delete_retries_once_after_401() {
    let server = MockServer::start().await;
    token_mock(&server).await;

    Mock::given(method("DELETE"))
        .and(path("/api/v1/tenants/t-del"))
        .respond_with(ResponseTemplate::new(401))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/tenants/t-del"))
        .respond_with(ResponseTemplate::new(204))
        .with_priority(2)
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    client
        .delete_tenant(&TenantId::new("t-del"))
        .await
        .expect("delete_tenant should succeed against the 204 mock");
}

#[tokio::test]
async fn token_exchange_non_401_failure_surfaces_http_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(503).set_body_string("upstream down"))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let err = client
        .get_tenant_status(&TenantId::new("t-x"))
        .await
        .expect_err("must error");
    match err {
        CloudError::HttpStatus { status, body } => {
            assert_eq!(status, 503);
            assert!(body.contains("upstream down"));
        },
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[tokio::test]
async fn cached_token_is_reused_across_two_calls() {
    let server = MockServer::start().await;
    // Token endpoint allowed at most once: a second exchange would 500 and
    // fail the test, proving the first token was cached and reused.
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "cached_bearer",
            "expires_in": 600
        })))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(500))
        .with_priority(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-cache/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "tenant_id": "t-cache", "status": "running" }
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let tid = TenantId::new("t-cache");
    let first = client.get_tenant_status(&tid).await.expect("first ok");
    let second = client.get_tenant_status(&tid).await.expect("second ok");
    assert_eq!(first.status, "running");
    assert_eq!(second.status, "running");
}

#[tokio::test]
async fn tenant_post_retries_once_after_401() {
    let server = MockServer::start().await;
    token_mock(&server).await;

    Mock::given(method("POST"))
        .and(path("/api/v1/tenants/t-dep/deploy"))
        .respond_with(ResponseTemplate::new(401))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/tenants/t-dep/deploy"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "status": "deploying", "app_url": "https://t-dep.fly.dev" }
        })))
        .with_priority(2)
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let response = client
        .deploy(&TenantId::new("t-dep"), "registry/image:tag")
        .await
        .expect("deploy retry should succeed");
    assert_eq!(response.status, "deploying");
}

#[tokio::test]
async fn tenant_put_retries_once_after_401() {
    let server = MockServer::start().await;
    token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path("/api/v1/tenants/t-ext/external-db-access"))
        .respond_with(ResponseTemplate::new(401))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/tenants/t-ext/external-db-access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "tenant_id": "t-ext",
                "external_db_access": true,
                "database_url": "postgres://u:p@db/demo"
            }
        })))
        .with_priority(2)
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let response = client
        .set_external_db_access(&TenantId::new("t-ext"), true)
        .await
        .expect("put retry should succeed");
    assert!(response.external_db_access);
}

#[tokio::test]
async fn tenant_put_no_content_retries_once_after_401() {
    let server = MockServer::start().await;
    token_mock(&server).await;

    Mock::given(method("PUT"))
        .and(path("/api/v1/tenants/t-sec/secrets"))
        .respond_with(ResponseTemplate::new(401))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/tenants/t-sec/secrets"))
        .respond_with(ResponseTemplate::new(204))
        .with_priority(2)
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let keys = client
        .set_secrets(
            &TenantId::new("t-sec"),
            std::collections::HashMap::from([("API_KEY".to_owned(), "v".to_owned())]),
        )
        .await
        .expect("set_secrets retry should succeed");
    assert_eq!(keys, vec!["API_KEY".to_owned()]);
}

#[tokio::test]
async fn tenant_post_empty_retries_once_after_401() {
    let server = MockServer::start().await;
    token_mock(&server).await;

    Mock::given(method("POST"))
        .and(path("/api/v1/tenants/t-restart/restart"))
        .respond_with(ResponseTemplate::new(401))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/tenants/t-restart/restart"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "status": "restarting" })))
        .with_priority(2)
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "op").unwrap();
    let response = client
        .restart_tenant(&TenantId::new("t-restart"))
        .await
        .expect("restart retry should succeed");
    assert_eq!(response.status, "restarting");
}
