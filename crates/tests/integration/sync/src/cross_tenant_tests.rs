//! Cross-tenant guard: the cloud-relay URL must encode the *caller's*
//! tenant, and a 403 from the cloud (the server-side ACL refusing a
//! cross-org write) must be a terminal error — never retried, never
//! silently downgraded, never fanned out to a sibling tenant.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use systemprompt_identifiers::TenantId;
use systemprompt_sync::api_client::RetryConfig;
use systemprompt_sync::{SyncApiClient, SyncError};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> SyncApiClient {
    SyncApiClient::new(&server.uri(), "operator-token")
        .expect("client")
        .with_retry_config(RetryConfig {
            max_attempts: 4,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(2),
            exponential_base: 2,
        })
}

#[tokio::test]
async fn upload_url_encodes_caller_tenant_not_a_sibling() {
    let server = MockServer::start().await;
    let calls_alpha = Arc::new(AtomicUsize::new(0));
    let calls_beta = Arc::new(AtomicUsize::new(0));

    {
        let c = calls_alpha.clone();
        Mock::given(method("POST"))
            .and(path("/api/v1/cloud/tenants/tenant-alpha/files"))
            .respond_with(move |_r: &wiremock::Request| {
                c.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "files_uploaded": 1 }))
            })
            .mount(&server)
            .await;
    }
    {
        let c = calls_beta.clone();
        Mock::given(method("POST"))
            .and(path("/api/v1/cloud/tenants/tenant-beta/files"))
            .respond_with(move |_r: &wiremock::Request| {
                c.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "files_uploaded": 99 }))
            })
            .mount(&server)
            .await;
    }

    client(&server)
        .upload_files(&TenantId::new("tenant-alpha"), vec![1])
        .await
        .expect("alpha upload");

    assert_eq!(calls_alpha.load(Ordering::SeqCst), 1);
    assert_eq!(
        calls_beta.load(Ordering::SeqCst),
        0,
        "upload for tenant-alpha must never reach the tenant-beta route"
    );
}

#[tokio::test]
async fn forbidden_tenant_is_terminal_no_retry() {
    let server = MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/tenants/tenant-cross/files"))
        .respond_with(move |_r: &wiremock::Request| {
            c.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(403).set_body_string("not a member of org")
        })
        .mount(&server)
        .await;

    let res = client(&server)
        .upload_files(&TenantId::new("tenant-cross"), vec![1])
        .await;
    assert!(
        matches!(res, Err(SyncError::ApiError { status: 403, .. })),
        "403 cross-org must surface as ApiError, got {res:?}",
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "403 must not be retried — retry would amplify a cross-tenant policy violation",
    );
}

#[tokio::test]
async fn unauthorized_relay_is_terminal_not_silently_replayed() {
    let server = MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/tenants/tenant-401/files"))
        .respond_with(move |_r: &wiremock::Request| {
            c.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(401).set_body_string("revoked operator token")
        })
        .mount(&server)
        .await;

    // No `with_direct_sync_origin` ⇒ no service-token mint path ⇒ 401
    // bubbles up immediately. (The direct-sync path's one-shot refresh is
    // covered by token_exchange_tests.)
    let res = client(&server)
        .upload_files(&TenantId::new("tenant-401"), vec![1])
        .await;
    assert!(
        matches!(res, Err(SyncError::Unauthorized)),
        "401 on cloud-relay must surface as Unauthorized, got {res:?}",
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
