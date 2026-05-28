//! Wiremock-driven tests for `SyncApiClient` HTTP paths that the existing
//! constructor-only suite does not exercise: registry-token / deploy /
//! upload / download / get-tenant-app-id / get-database-url.

use std::time::Duration;
use systemprompt_identifiers::TenantId;
use systemprompt_sync::SyncApiClient;
use systemprompt_sync::api_client::RetryConfig;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fast_retry() -> RetryConfig {
    RetryConfig {
        max_attempts: 2,
        initial_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(2),
        exponential_base: 2,
    }
}

#[tokio::test]
async fn upload_files_success_returns_count() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/tenants/t1/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files_uploaded": 7
        })))
        .mount(&server)
        .await;

    let client = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry());
    let tenant = TenantId::new("t1");
    let res = client
        .upload_files(&tenant, vec![1, 2, 3])
        .await
        .expect("upload");
    assert_eq!(res.files_uploaded, 7);
}

#[tokio::test]
async fn upload_files_404_propagates_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/tenants/t1/files"))
        .respond_with(ResponseTemplate::new(404).set_body_string("nope"))
        .mount(&server)
        .await;

    let client = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry());
    let tenant = TenantId::new("t1");
    let err = client.upload_files(&tenant, vec![]).await.expect_err("404");
    assert!(format!("{err}").to_lowercase().contains("404"));
}

#[tokio::test]
async fn download_files_returns_bytes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t2/files"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"tarball-bytes".to_vec()))
        .mount(&server)
        .await;

    let client = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry());
    let tenant = TenantId::new("t2");
    let bytes = client.download_files(&tenant).await.expect("download");
    assert_eq!(bytes, b"tarball-bytes");
}

#[tokio::test]
async fn get_registry_token_parses_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t1/registry-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "registry": "registry.fly.io",
            "username": "u",
            "token": "abc"
        })))
        .mount(&server)
        .await;

    let client = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry());
    let tenant = TenantId::new("t1");
    let res = client.get_registry_token(&tenant).await.expect("token");
    assert_eq!(res.registry, "registry.fly.io");
    assert_eq!(res.username, "u");
    assert_eq!(res.token, "abc");
}

#[tokio::test]
async fn deploy_parses_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/tenants/t1/deploy"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "ok",
            "app_url": "https://app.fly.dev"
        })))
        .mount(&server)
        .await;

    let client = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry());
    let tenant = TenantId::new("t1");
    let res = client
        .deploy(&tenant, "registry.fly.io/app:tag")
        .await
        .expect("deploy");
    assert_eq!(res.status, "ok");
    assert_eq!(res.app_url.as_deref(), Some("https://app.fly.dev"));
}

#[tokio::test]
async fn get_tenant_app_id_returns_app_name() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fly_app_name": "my-app"
        })))
        .mount(&server)
        .await;

    let client = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry());
    let tenant = TenantId::new("t1");
    let id = client.get_tenant_app_id(&tenant).await.expect("app id");
    assert_eq!(id, "my-app");
}

#[tokio::test]
async fn get_tenant_app_id_missing_returns_no_app_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fly_app_name": null
        })))
        .mount(&server)
        .await;

    let client = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry());
    let tenant = TenantId::new("t1");
    let err = client
        .get_tenant_app_id(&tenant)
        .await
        .expect_err("expect no app");
    assert!(
        format!("{err}").to_lowercase().contains("tenant")
            || format!("{err}").to_lowercase().contains("app")
    );
}

#[tokio::test]
async fn get_database_url_returns_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t1/database"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "database_url": "postgres://db.fly.dev/app"
        })))
        .mount(&server)
        .await;

    let client = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry());
    let tenant = TenantId::new("t1");
    let url = client.get_database_url(&tenant).await.expect("db url");
    assert_eq!(url, "postgres://db.fly.dev/app");
}

#[tokio::test]
async fn get_database_url_missing_returns_404_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t1/database"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "database_url": null
        })))
        .mount(&server)
        .await;

    let client = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry());
    let tenant = TenantId::new("t1");
    let err = client
        .get_database_url(&tenant)
        .await
        .expect_err("missing url errors");
    assert!(
        format!("{err}").contains("404") || format!("{err}").to_lowercase().contains("database")
    );
}

#[tokio::test]
async fn upload_files_with_direct_sync_uses_direct_origin() {
    let server = MockServer::start().await;
    // The direct-sync path first exchanges the operator token for a
    // service JWT via RFC 8693 against `/api/v1/core/oauth/token`.
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "service-jwt",
            "token_type": "Bearer"
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/sync/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files_uploaded": 3
        })))
        .mount(&server)
        .await;

    let origin = server.uri();
    let client = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_direct_sync_origin(Some(origin))
        .with_retry_config(fast_retry());
    let tenant = TenantId::new("does-not-matter");
    let res = client
        .upload_files(&tenant, vec![1, 2, 3])
        .await
        .expect("upload via direct sync");
    assert_eq!(res.files_uploaded, 3);
}
