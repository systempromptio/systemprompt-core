//! Heartbeat loop tests: `run_loop` driven under a paused tokio clock against
//! a wiremock gateway, covering the success POST, the 401 cache-invalidation
//! branch, the non-2xx warn branch, and the no-credential auth-error branch.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use arc_swap::ArcSwap;
use systemprompt_bridge::auth::types::HelperOutput;
use systemprompt_bridge::config::{RuntimeConfig, SharedRuntimeConfig};
use systemprompt_bridge::ids::BearerToken;
use systemprompt_bridge::proxy::ProxyStats;
use systemprompt_bridge::proxy::heartbeat::run_loop;
use systemprompt_bridge::proxy::session::SessionContext;
use systemprompt_bridge::proxy::token_cache::TokenCache;
use systemprompt_identifiers::ValidatedUrl;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn runtime_config(uri: &str) -> SharedRuntimeConfig {
    Arc::new(ArcSwap::from_pointee(RuntimeConfig {
        gateway_base: Arc::new(ValidatedUrl::new(uri)),
    }))
}

fn counting_cache(refresh_calls: Arc<AtomicU32>) -> Arc<TokenCache> {
    Arc::new(TokenCache::new(Arc::new(move |_threshold| {
        let refresh_calls = Arc::clone(&refresh_calls);
        Box::pin(async move {
            refresh_calls.fetch_add(1, Ordering::SeqCst);
            Some(HelperOutput {
                token: BearerToken::new("heartbeat-bearer"),
                ttl: 3600,
                headers: std::collections::HashMap::new(),
            })
        })
    })))
}

fn empty_cache() -> Arc<TokenCache> {
    Arc::new(TokenCache::new(Arc::new(|_threshold| {
        Box::pin(async { None })
    })))
}

async fn wait_for_requests(server: &MockServer, at_least: usize) -> Vec<Request> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while std::time::Instant::now() < deadline {
        let received = server.received_requests().await.unwrap_or_default();
        if received.len() >= at_least {
            return received;
        }
        tokio::time::sleep(std::time::Duration::from_secs(31)).await;
        tokio::task::yield_now().await;
    }
    panic!("gateway never received {at_least} heartbeat(s)");
}

#[tokio::test(start_paused = true)]
async fn heartbeat_posts_payload_with_bearer() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/bridge/heartbeat"))
        .and(header("authorization", "Bearer heartbeat-bearer"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let refresh_calls = Arc::new(AtomicU32::new(0));
    let session = Arc::new(SessionContext::new());
    session.touch_activity();
    let stats = Arc::new(ProxyStats::default());
    stats.forwarded_total.store(7, Ordering::Relaxed);

    let handle = tokio::spawn(run_loop(
        runtime_config(&server.uri()),
        counting_cache(Arc::clone(&refresh_calls)),
        Arc::clone(&session),
        Arc::clone(&stats),
        reqwest::Client::new(),
    ));

    let received = wait_for_requests(&server, 1).await;
    handle.abort();

    let body: serde_json::Value = serde_json::from_slice(&received[0].body).unwrap();
    assert_eq!(body["session_id"], session.session_id().as_str());
    assert_eq!(body["forwarded_total"], 7);
    assert_eq!(body["os"], std::env::consts::OS);
    assert!(body["last_activity_at"].is_string());
    assert!(refresh_calls.load(Ordering::SeqCst) >= 1);
}

struct UnauthorizedOnce {
    calls: AtomicU32,
}

impl Respond for UnauthorizedOnce {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            ResponseTemplate::new(401)
        } else {
            ResponseTemplate::new(200)
        }
    }
}

#[tokio::test(start_paused = true)]
async fn heartbeat_401_invalidates_token_cache() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/bridge/heartbeat"))
        .respond_with(UnauthorizedOnce {
            calls: AtomicU32::new(0),
        })
        .mount(&server)
        .await;

    let refresh_calls = Arc::new(AtomicU32::new(0));
    let handle = tokio::spawn(run_loop(
        runtime_config(&server.uri()),
        counting_cache(Arc::clone(&refresh_calls)),
        Arc::new(SessionContext::new()),
        Arc::new(ProxyStats::default()),
        reqwest::Client::new(),
    ));

    wait_for_requests(&server, 2).await;
    handle.abort();

    assert!(
        refresh_calls.load(Ordering::SeqCst) >= 2,
        "401 must invalidate the cache so the next tick re-authenticates \
         (refresh calls: {})",
        refresh_calls.load(Ordering::SeqCst)
    );
}

#[tokio::test(start_paused = true)]
async fn heartbeat_server_error_keeps_looping() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/bridge/heartbeat"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let refresh_calls = Arc::new(AtomicU32::new(0));
    let handle = tokio::spawn(run_loop(
        runtime_config(&server.uri()),
        counting_cache(refresh_calls),
        Arc::new(SessionContext::new()),
        Arc::new(ProxyStats::default()),
        reqwest::Client::new(),
    ));

    let received = wait_for_requests(&server, 2).await;
    handle.abort();
    assert!(received.len() >= 2, "loop must survive upstream errors");
}

#[tokio::test(start_paused = true)]
async fn heartbeat_without_credentials_sends_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/bridge/heartbeat"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let handle = tokio::spawn(run_loop(
        runtime_config(&server.uri()),
        empty_cache(),
        Arc::new(SessionContext::new()),
        Arc::new(ProxyStats::default()),
        reqwest::Client::new(),
    ));

    for _ in 0..5 {
        tokio::time::sleep(std::time::Duration::from_secs(31)).await;
    }
    handle.abort();

    let received = server.received_requests().await.unwrap_or_default();
    assert!(
        received.is_empty(),
        "auth-unavailable ticks must not POST heartbeats"
    );
}
