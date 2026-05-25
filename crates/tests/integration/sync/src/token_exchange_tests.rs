//! End-to-end RFC 8693 token exchange via
//! `SyncApiClient::with_direct_sync_origin`.
//!
//! The hardening fix introduced `with_direct_sync_origin` so a
//! plain-HTTP `MockServer` can stand in for the deployment that issues
//! the service JWT. Production callers continue to use `with_direct_sync`
//! which pins `https://`.
//!
//! These tests exercise the *real* `bearer_token` + `upload_files` path:
//! the client must (a) mint a service JWT, (b) cache it across calls,
//! (c) refresh it exactly once on a 401, (d) propagate the new token on
//! the retried upload, and (e) chain the operator token as the
//! `subject_token` (act_chain: operator → service → deploy scope).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use systemprompt_identifiers::TenantId;
use systemprompt_sync::SyncApiClient;
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn client_against(server: &MockServer, operator_token: &str) -> SyncApiClient {
    SyncApiClient::new(&server.uri(), operator_token)
        .expect("client")
        .with_direct_sync_origin(Some(server.uri()))
}

#[derive(Clone, Default)]
struct CallCounter(Arc<AtomicUsize>);

impl CallCounter {
    fn new() -> Self {
        Self::default()
    }
    fn count(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Clone)]
struct UploadResponder {
    fail_first_n_with_401: usize,
    expected_tokens: Vec<String>,
    seen_tokens: Arc<std::sync::Mutex<Vec<String>>>,
    calls: CallCounter,
}

impl Respond for UploadResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let n = self.calls.0.fetch_add(1, Ordering::SeqCst);
        let auth = request
            .headers
            .get("authorization")
            .map(|v| v.to_str().unwrap_or("").to_owned())
            .unwrap_or_default();
        self.seen_tokens.lock().unwrap().push(auth.clone());

        if n < self.fail_first_n_with_401 {
            return ResponseTemplate::new(401).set_body_string("stale token");
        }
        let expected = self
            .expected_tokens
            .get(n)
            .or_else(|| self.expected_tokens.last())
            .cloned()
            .unwrap_or_default();
        if !auth.contains(&expected) {
            return ResponseTemplate::new(401).set_body_string("token mismatch");
        }
        ResponseTemplate::new(200).set_body_json(serde_json::json!({ "files_uploaded": 1 }))
    }
}

#[tokio::test]
async fn token_exchange_uses_operator_token_as_subject_token() {
    let server = MockServer::start().await;
    let exchange_calls = CallCounter::new();
    let exchange_calls_clone = exchange_calls.clone();

    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .and(header("content-type", "application/x-www-form-urlencoded"))
        .and(body_string_contains(
            "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Atoken-exchange",
        ))
        .and(body_string_contains("subject_token=operator-jwt"))
        .and(body_string_contains(
            "subject_token_type=urn%3Aietf%3Aparams%3Aoauth%3Atoken-type%3Ajwt",
        ))
        .respond_with(move |_req: &Request| {
            exchange_calls_clone.0.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "service-jwt-A"
            }))
        })
        .mount(&server)
        .await;

    let uploads = UploadResponder {
        fail_first_n_with_401: 0,
        expected_tokens: vec!["service-jwt-A".into()],
        seen_tokens: Arc::new(std::sync::Mutex::new(Vec::new())),
        calls: CallCounter::new(),
    };
    Mock::given(method("POST"))
        .and(path("/api/v1/sync/files"))
        .respond_with(uploads.clone())
        .mount(&server)
        .await;

    let client = client_against(&server, "operator-jwt");
    let tenant = TenantId::new("tenant-token-exchange");
    let res = client.upload_files(&tenant, vec![1, 2, 3]).await;
    assert!(res.is_ok(), "upload should succeed: {res:?}");
    assert_eq!(res.unwrap().files_uploaded, 1);

    assert_eq!(
        exchange_calls.count(),
        1,
        "service JWT exchanged exactly once"
    );
    let seen = uploads.seen_tokens.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        seen[0], "Bearer service-jwt-A",
        "deploy upload must carry the exchanged service JWT, not the operator token"
    );
}

#[tokio::test]
async fn service_jwt_is_cached_across_uploads() {
    let server = MockServer::start().await;
    let exchange_calls = CallCounter::new();
    let exchange_calls_clone = exchange_calls.clone();

    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(move |_req: &Request| {
            exchange_calls_clone.0.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "service-jwt-cached"
            }))
        })
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/sync/files"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "files_uploaded": 1 })),
        )
        .mount(&server)
        .await;

    let client = client_against(&server, "op");
    let tenant = TenantId::new("tenant-cache");
    for _ in 0..3 {
        client
            .upload_files(&tenant, vec![0])
            .await
            .expect("upload ok");
    }
    assert_eq!(
        exchange_calls.count(),
        1,
        "cached service JWT must be reused across calls (no exchange storm)"
    );
}

#[tokio::test]
async fn unauthorized_upload_triggers_single_token_refresh_then_retries() {
    let server = MockServer::start().await;

    // Exchange returns a distinct token each call so we can prove refresh happened.
    let exchange_seq = Arc::new(std::sync::Mutex::new(vec![
        "service-jwt-old".to_string(),
        "service-jwt-new".to_string(),
    ]));
    let exchange_calls = CallCounter::new();
    let exchange_calls_clone = exchange_calls.clone();
    let exchange_seq_clone = exchange_seq.clone();
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(move |_req: &Request| {
            exchange_calls_clone.0.fetch_add(1, Ordering::SeqCst);
            let mut q = exchange_seq_clone.lock().unwrap();
            let tok = if q.is_empty() {
                "service-jwt-tail".to_string()
            } else {
                q.remove(0)
            };
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "access_token": tok }))
        })
        .mount(&server)
        .await;

    let uploads = UploadResponder {
        fail_first_n_with_401: 1,
        expected_tokens: vec!["service-jwt-old".into(), "service-jwt-new".into()],
        seen_tokens: Arc::new(std::sync::Mutex::new(Vec::new())),
        calls: CallCounter::new(),
    };
    Mock::given(method("POST"))
        .and(path("/api/v1/sync/files"))
        .respond_with(uploads.clone())
        .mount(&server)
        .await;

    let client = client_against(&server, "op");
    let tenant = TenantId::new("tenant-refresh");
    let res = client.upload_files(&tenant, vec![42]).await;
    assert!(
        res.is_ok(),
        "401 then retry with refreshed token must succeed: {res:?}"
    );

    assert_eq!(
        exchange_calls.count(),
        2,
        "exchange called twice: initial mint + one forced refresh on 401"
    );
    let seen = uploads.seen_tokens.lock().unwrap().clone();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0], "Bearer service-jwt-old");
    assert_eq!(seen[1], "Bearer service-jwt-new");
}

#[tokio::test]
async fn token_exchange_failure_bubbles_up_to_caller() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(403).set_body_string("scope intersection empty"))
        .mount(&server)
        .await;

    let client = client_against(&server, "op");
    let tenant = TenantId::new("tenant-exchange-fail");
    let res = client.upload_files(&tenant, vec![1]).await;
    assert!(
        res.is_err(),
        "exchange 403 must propagate, not silently fall through to anonymous upload"
    );
}
