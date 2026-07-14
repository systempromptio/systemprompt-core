//! Retry and direct-sync token-remint behaviour of `SyncApiClient`:
//! retryable-status retries, the zero-attempt exhaustion guard, 401 remint
//! on the direct-sync path, token caching, and remint-once semantics.

use std::time::Duration;

use serde_json::json;
use systemprompt_identifiers::TenantId;
use systemprompt_sync::api_client::RetryConfig;
use systemprompt_sync::{SyncApiClient, SyncError};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fast_retry(max_attempts: u32) -> RetryConfig {
    RetryConfig {
        max_attempts,
        initial_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(2),
        exponential_base: 2,
    }
}

fn tenant() -> TenantId {
    TenantId::new("t-retry")
}

#[tokio::test]
async fn upload_files_retries_after_retryable_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/tenants/t-retry/files"))
        .respond_with(ResponseTemplate::new(503).set_body_string("busy"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/tenants/t-retry/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "files_uploaded": 4 })))
        .expect(1)
        .mount(&server)
        .await;

    let client = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry(3));
    let upload = client
        .upload_files(&tenant(), b"payload".to_vec())
        .await
        .expect("upload after retry");
    assert_eq!(upload.files_uploaded, 4);
}

#[tokio::test]
async fn download_files_returns_final_retryable_error_when_attempts_exhaust() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t-retry/files"))
        .respond_with(ResponseTemplate::new(503).set_body_string("still busy"))
        .expect(2)
        .mount(&server)
        .await;

    let client = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry(2));
    let err = client
        .download_files(&tenant())
        .await
        .expect_err("must fail");
    match err {
        SyncError::ApiError { status, message } => {
            assert_eq!(status, 503);
            assert_eq!(message, "still busy");
        },
        other => panic!("expected ApiError, got {other:?}"),
    }
}

#[tokio::test]
async fn zero_max_attempts_short_circuits_without_any_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t-retry/files"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let client = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry(0));
    let err = client
        .download_files(&tenant())
        .await
        .expect_err("must fail");
    match err {
        SyncError::ApiError { status, message } => {
            assert_eq!(status, 503);
            assert_eq!(message, "Max retry attempts exceeded");
        },
        other => panic!("expected ApiError, got {other:?}"),
    }
}

async fn mount_exchange(server: &MockServer, token: &str, times: u64) {
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "access_token": token })))
        .up_to_n_times(times)
        .mount(server)
        .await;
}

#[tokio::test]
async fn direct_sync_remints_bearer_once_after_401() {
    let server = MockServer::start().await;
    mount_exchange(&server, "stale", 1).await;
    mount_exchange(&server, "fresh", 1).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/sync/files"))
        .and(header("Authorization", "Bearer stale"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/sync/files"))
        .and(header("Authorization", "Bearer fresh"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"tarball".to_vec()))
        .expect(1)
        .mount(&server)
        .await;

    let client = SyncApiClient::new(&server.uri(), "operator-token")
        .expect("client")
        .with_direct_sync_origin(Some(server.uri()))
        .with_retry_config(fast_retry(3));

    let data = client.download_files(&tenant()).await.expect("download");
    assert_eq!(data, b"tarball");
}

#[tokio::test]
async fn direct_sync_reuses_cached_bearer_across_calls() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "access_token": "cached" })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/sync/files"))
        .and(header("Authorization", "Bearer cached"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"x".to_vec()))
        .expect(2)
        .mount(&server)
        .await;

    let client = SyncApiClient::new(&server.uri(), "operator-token")
        .expect("client")
        .with_direct_sync_origin(Some(server.uri()))
        .with_retry_config(fast_retry(3));

    client.download_files(&tenant()).await.expect("first");
    client.download_files(&tenant()).await.expect("second");
}

#[tokio::test]
async fn direct_sync_gives_up_after_second_unauthorized() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "access_token": "rejected" })),
        )
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/sync/files"))
        .respond_with(ResponseTemplate::new(401))
        .expect(2)
        .mount(&server)
        .await;

    let client = SyncApiClient::new(&server.uri(), "operator-token")
        .expect("client")
        .with_direct_sync_origin(Some(server.uri()))
        .with_retry_config(fast_retry(5));

    let err = client
        .download_files(&tenant())
        .await
        .expect_err("must fail");
    assert!(matches!(err, SyncError::ApiError { status: 401, .. }));
}
