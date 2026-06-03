use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use arc_swap::ArcSwap;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use systemprompt_bridge::auth::types::HelperOutput;
use systemprompt_bridge::config::{Config, RuntimeConfig, SharedRuntimeConfig};
use systemprompt_bridge::ids::{BearerToken, ProxySecret};
use systemprompt_bridge::proxy::dispatch::handle_request;
use systemprompt_bridge::proxy::server::{ProxyContext, ProxyStats};
use systemprompt_bridge::proxy::session::SessionContext;
use systemprompt_bridge::proxy::token_cache::{RefreshFn, TokenCache};
use systemprompt_identifiers::ValidatedUrl;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SECRET: &str = "test-loopback-secret";

/// A live, listening proxy instance plus the mock upstream gateway it forwards
/// to. Both bindings must be kept alive for the duration of a test.
struct Harness {
    port: u16,
    gateway: MockServer,
    stats: Arc<ProxyStats>,
}

fn stub_refresh() -> RefreshFn {
    Arc::new(|_threshold| {
        Box::pin(async {
            Some(HelperOutput {
                token: BearerToken::new("upstream-jwt"),
                ttl: 3600,
                headers: Default::default(),
            })
        })
    })
}

fn shared_runtime_config(gateway_uri: &str) -> SharedRuntimeConfig {
    let cfg = Config {
        gateway_url: Some(ValidatedUrl::new(gateway_uri)),
        ..Default::default()
    };
    Arc::new(ArcSwap::from_pointee(RuntimeConfig::from_config(&cfg)))
}

async fn spawn_harness() -> Harness {
    let gateway = MockServer::start().await;

    let stats = Arc::new(ProxyStats::default());
    let ctx = ProxyContext {
        runtime_config: shared_runtime_config(&gateway.uri()),
        secret: Arc::new(ProxySecret::new(SECRET)),
        stats: Arc::clone(&stats),
        client: reqwest::Client::new(),
        token_cache: Arc::new(TokenCache::new(stub_refresh())),
        session: Arc::new(SessionContext::new()),
    };

    let addr: SocketAddr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = TcpListener::bind(addr).await.expect("bind 127.0.0.1:0");
    let port = listener.local_addr().expect("local_addr").port();

    tokio::spawn(async move {
        loop {
            let Ok((stream, peer)) = listener.accept().await else {
                break;
            };
            let conn_ctx = ctx.clone();
            tokio::spawn(async move {
                let io = TokioIo::new(stream);
                let svc = service_fn(move |req| handle_request(req, conn_ctx.clone(), peer));
                let _ = http1::Builder::new()
                    .keep_alive(false)
                    .serve_connection(io, svc)
                    .await;
            });
        }
    });

    Harness {
        port,
        gateway,
        stats,
    }
}

impl Harness {
    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("client build")
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn happy_path_forwards_to_gateway_and_records_stats() {
    let h = spawn_harness().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(r#"{"ok":true,"echo":"upstream"}"#),
        )
        .mount(&h.gateway)
        .await;

    let resp = Harness::client()
        .post(h.url("/v1/messages"))
        .header("authorization", format!("Bearer {SECRET}"))
        .header("content-type", "application/json")
        .body(r#"{"messages":[{"role":"user","content":"hi"}]}"#)
        .send()
        .await
        .expect("request to proxy");

    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.expect("read body");
    assert_eq!(body, r#"{"ok":true,"echo":"upstream"}"#);

    assert_eq!(h.stats.forwarded_total.load(Ordering::Relaxed), 1);
    assert_eq!(h.stats.last_status.load(Ordering::Relaxed), 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_authorization_is_rejected_403() {
    let h = spawn_harness().await;

    let resp = Harness::client()
        .post(h.url("/v1/messages"))
        .header("content-type", "application/json")
        .body("{}")
        .send()
        .await
        .expect("request to proxy");

    assert_eq!(resp.status().as_u16(), 403);
    let body = resp.text().await.expect("read body");
    assert!(
        body.contains("bad loopback secret"),
        "expected bad-secret body, got: {body}"
    );
    assert_eq!(h.stats.forwarded_total.load(Ordering::Relaxed), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wrong_secret_is_rejected_403() {
    let h = spawn_harness().await;

    let resp = Harness::client()
        .post(h.url("/v1/messages"))
        .header("authorization", "Bearer not-the-secret")
        .body("{}")
        .send()
        .await
        .expect("request to proxy");

    assert_eq!(resp.status().as_u16(), 403);
    let body = resp.text().await.expect("read body");
    assert!(
        body.contains("bad loopback secret"),
        "expected bad-secret body, got: {body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_loopback_host_is_rejected_403() {
    let h = spawn_harness().await;

    let resp = Harness::client()
        .post(h.url("/v1/messages"))
        .header("host", "evil.example.com")
        .header("authorization", format!("Bearer {SECRET}"))
        .body("{}")
        .send()
        .await
        .expect("request to proxy");

    assert_eq!(resp.status().as_u16(), 403);
    let body = resp.text().await.expect("read body");
    assert!(
        body.contains("non-loopback host"),
        "expected non-loopback-host body, got: {body}"
    );
    assert_eq!(h.stats.forwarded_total.load(Ordering::Relaxed), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_mcp_server_yields_404() {
    let h = spawn_harness().await;

    let resp = Harness::client()
        .post(h.url("/mcp/does-not-exist"))
        .header("authorization", format!("Bearer {SECRET}"))
        .body("{}")
        .send()
        .await
        .expect("request to proxy");

    assert_eq!(resp.status().as_u16(), 404);
    let body = resp.text().await.expect("read body");
    assert!(
        body.contains("unknown managed MCP server"),
        "expected unknown-mcp body, got: {body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upstream_500_is_forwarded_and_recorded() {
    let h = spawn_harness().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(500).set_body_string("upstream boom"))
        .mount(&h.gateway)
        .await;

    let resp = Harness::client()
        .post(h.url("/v1/messages"))
        .header("authorization", format!("Bearer {SECRET}"))
        .body(r#"{"messages":[{"role":"user","content":"x"}]}"#)
        .send()
        .await
        .expect("request to proxy");

    assert_eq!(
        resp.status().as_u16(),
        500,
        "forward.rs passes the upstream status through verbatim"
    );
    let body = resp.text().await.expect("read body");
    assert_eq!(body, "upstream boom");

    assert_eq!(h.stats.forwarded_total.load(Ordering::Relaxed), 1);
    assert_eq!(h.stats.last_status.load(Ordering::Relaxed), 500);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sse_messages_response_streams_back_and_taps_usage() {
    let h = spawn_harness().await;

    let sse = concat!(
        "event: message_start\n",
        "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":11,\"output_tokens\":0}}}\n\n",
        "event: message_delta\n",
        "data: {\"type\":\"message_delta\",\"usage\":{\"input_tokens\":11,\"output_tokens\":7}}\n\n",
        "data: [DONE]\n\n",
    );

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(sse.as_bytes(), "text/event-stream"),
        )
        .mount(&h.gateway)
        .await;

    let resp = Harness::client()
        .post(h.url("/v1/messages"))
        .header("authorization", format!("Bearer {SECRET}"))
        .header("content-type", "application/json")
        .body(r#"{"messages":[{"role":"user","content":"stream"}],"stream":true}"#)
        .send()
        .await
        .expect("request to proxy");

    assert_eq!(resp.status().as_u16(), 200);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(
        ct.contains("text/event-stream"),
        "content-type preserved, got: {ct}"
    );
    let body = resp.text().await.expect("read streamed body");
    assert!(body.contains("message_start"), "body forwarded: {body}");
    assert!(body.contains("[DONE]"));

    for _ in 0..50 {
        if h.stats.messages_total.load(Ordering::Relaxed) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(h.stats.messages_total.load(Ordering::Relaxed), 1);
    assert_eq!(h.stats.tokens_in_total.load(Ordering::Relaxed), 11);
    assert_eq!(h.stats.tokens_out_total.load(Ordering::Relaxed), 7);
}
