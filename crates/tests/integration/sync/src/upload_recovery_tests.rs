//! Partial-deploy + transient-failure recovery for the cloud-relay upload
//! path. The cloud-relay protocol packages the entire `services/` tree
//! into one tarball POST, so the failure surface is "upload N bytes,
//! peer 502s, retry, succeed" rather than "K of N files uploaded".
//!
//! Tests assert:
//! - retryable 5xx / 429 are retried up to `max_attempts` with the same tarball
//!   bytes (idempotent),
//! - non-retryable 4xx aborts immediately (no exponential thrash),
//! - the request body is byte-identical across retries (a resumed sync must not
//!   corrupt the bundle), and
//! - after the configured cap the client surfaces a 503 instead of looping
//!   forever.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use systemprompt_identifiers::TenantId;
use systemprompt_sync::api_client::RetryConfig;
use systemprompt_sync::{SyncApiClient, SyncError};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn fast_retry() -> RetryConfig {
    RetryConfig {
        max_attempts: 4,
        initial_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(5),
        exponential_base: 2,
    }
}

fn cloud_relay_client(server: &MockServer) -> SyncApiClient {
    SyncApiClient::new(&server.uri(), "operator-token")
        .expect("client")
        .with_retry_config(fast_retry())
}

#[derive(Clone)]
struct BodyCapture {
    calls: Arc<AtomicUsize>,
    bodies: Arc<Mutex<Vec<Vec<u8>>>>,
    status_seq: Arc<Mutex<Vec<u16>>>,
}

impl Respond for BodyCapture {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.bodies.lock().unwrap().push(request.body.clone());
        let status = {
            let mut q = self.status_seq.lock().unwrap();
            if q.is_empty() { 200 } else { q.remove(0) }
        };
        match status {
            200 => {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "files_uploaded": 7 }))
            },
            other => ResponseTemplate::new(other).set_body_string(format!("status {other}")),
        }
    }
}

fn body_capture(status_seq: Vec<u16>) -> BodyCapture {
    BodyCapture {
        calls: Arc::new(AtomicUsize::new(0)),
        bodies: Arc::new(Mutex::new(Vec::new())),
        status_seq: Arc::new(Mutex::new(status_seq)),
    }
}

#[tokio::test]
async fn transient_502_is_retried_until_success() {
    let server = MockServer::start().await;
    let responder = body_capture(vec![502, 502, 200]);
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/tenants/tenant-recover/files"))
        .respond_with(responder.clone())
        .mount(&server)
        .await;

    let client = cloud_relay_client(&server);
    let payload = b"tarball-bytes-v1".to_vec();
    let res = client
        .upload_files(&TenantId::new("tenant-recover"), payload.clone())
        .await;
    assert!(res.is_ok(), "expected eventual success: {res:?}");
    assert_eq!(res.unwrap().files_uploaded, 7);

    assert_eq!(
        responder.calls.load(Ordering::SeqCst),
        3,
        "two 502s should produce exactly two retries"
    );
}

#[tokio::test]
async fn retried_uploads_resend_identical_body() {
    let server = MockServer::start().await;
    let responder = body_capture(vec![503, 503, 200]);
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/tenants/tenant-idem/files"))
        .respond_with(responder.clone())
        .mount(&server)
        .await;

    let client = cloud_relay_client(&server);
    let payload = b"deterministic-tarball-bytes".to_vec();
    client
        .upload_files(&TenantId::new("tenant-idem"), payload.clone())
        .await
        .expect("eventual success");

    let bodies = responder.bodies.lock().unwrap().clone();
    assert_eq!(bodies.len(), 3, "expected 3 attempts");
    for (idx, body) in bodies.iter().enumerate() {
        assert_eq!(
            body, &payload,
            "attempt {idx} sent a body that drifted from the original; resume must be idempotent",
        );
    }
}

#[tokio::test]
async fn non_retryable_400_aborts_immediately() {
    let server = MockServer::start().await;
    let responder = body_capture(vec![400]);
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/tenants/tenant-4xx/files"))
        .respond_with(responder.clone())
        .mount(&server)
        .await;

    let client = cloud_relay_client(&server);
    let res = client
        .upload_files(&TenantId::new("tenant-4xx"), vec![1, 2])
        .await;

    assert!(matches!(res, Err(SyncError::ApiError { status: 400, .. })));
    assert_eq!(
        responder.calls.load(Ordering::SeqCst),
        1,
        "400 must not be retried"
    );
}

#[tokio::test]
async fn exhausted_retries_surface_503_after_cap() {
    let server = MockServer::start().await;
    let responder = body_capture(vec![503, 503, 503, 503, 503]);
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/tenants/tenant-cap/files"))
        .respond_with(responder.clone())
        .mount(&server)
        .await;

    let client = cloud_relay_client(&server);
    let res = client
        .upload_files(&TenantId::new("tenant-cap"), vec![1])
        .await;
    let err = res.expect_err("must surface terminal error");
    match err {
        SyncError::ApiError { status, .. } => assert_eq!(
            status, 503,
            "exhausted-retry sentinel must be 503, not the last upstream status"
        ),
        other => panic!("expected ApiError 503, got {other:?}"),
    }
    assert_eq!(
        responder.calls.load(Ordering::SeqCst),
        4,
        "exactly max_attempts attempts"
    );
}
