use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
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
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SECRET: &str = "test-loopback-secret";

struct Harness {
    port: u16,
    gateway: MockServer,
    stats: Arc<ProxyStats>,
    mints: Arc<AtomicUsize>,
}

fn counting_refresh(mints: &Arc<AtomicUsize>) -> RefreshFn {
    let mints = Arc::clone(mints);
    Arc::new(move |_threshold| {
        let n = mints.fetch_add(1, Ordering::Relaxed) + 1;
        Box::pin(async move {
            Some(HelperOutput {
                token: BearerToken::new(format!("upstream-jwt-{n}")),
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

async fn spawn_with_base(gateway: MockServer, base: Option<String>) -> Harness {
    let stats = Arc::new(ProxyStats::default());
    let mints = Arc::new(AtomicUsize::new(0));
    let base = base.unwrap_or_else(|| gateway.uri());
    let ctx = ProxyContext {
        runtime_config: shared_runtime_config(&base),
        secret: Arc::new(ProxySecret::new(SECRET)),
        stats: Arc::clone(&stats),
        client: reqwest::Client::new(),
        token_cache: Arc::new(TokenCache::new(counting_refresh(&mints))),
        session: Arc::new(SessionContext::new()),
    };

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind 127.0.0.1:0");
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
        mints,
    }
}

async fn spawn_harness() -> Harness {
    spawn_with_base(MockServer::start().await, None).await
}

impl Harness {
    fn url(&self, p: &str) -> String {
        format!("http://127.0.0.1:{}{p}", self.port)
    }

    fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("client build")
    }

    async fn authed_post(&self, p: &str, body: &'static str) -> reqwest::Response {
        Self::client()
            .post(self.url(p))
            .header("authorization", format!("Bearer {SECRET}"))
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .expect("request to proxy")
    }

    async fn upstream_requests(&self) -> Vec<wiremock::Request> {
        self.gateway
            .received_requests()
            .await
            .expect("recorded requests")
    }
}

fn bearer_of(req: &wiremock::Request) -> String {
    req.headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_upstream_401_invalidates_the_cached_jwt_so_the_next_request_remints() {
    let h = spawn_harness().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(401).set_body_string("nope"))
        .mount(&h.gateway)
        .await;

    let first = h.authed_post("/v1/messages", r#"{"messages":[]}"#).await;
    assert_eq!(first.status().as_u16(), 401);
    let second = h.authed_post("/v1/messages", r#"{"messages":[]}"#).await;
    assert_eq!(second.status().as_u16(), 401);

    assert_eq!(
        h.mints.load(Ordering::Relaxed),
        2,
        "the 401 dropped the cached token, forcing a fresh mint"
    );
    let requests = h.upstream_requests().await;
    assert_eq!(bearer_of(&requests[0]), "Bearer upstream-jwt-1");
    assert_eq!(
        bearer_of(&requests[1]),
        "Bearer upstream-jwt-2",
        "the second attempt carries the re-minted JWT"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_successful_response_keeps_the_cached_jwt() {
    let h = spawn_harness().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(br#"{"ok":true}"#.to_vec(), "application/json"),
        )
        .mount(&h.gateway)
        .await;

    h.authed_post("/v1/messages", r#"{"messages":[]}"#).await;
    h.authed_post("/v1/messages", r#"{"messages":[]}"#).await;

    assert_eq!(
        h.mints.load(Ordering::Relaxed),
        1,
        "a healthy upstream leaves the cached token in place"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_json_messages_response_is_tapped_for_usage() {
    let h = spawn_harness().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            br#"{"usage":{"input_tokens":21,"output_tokens":4}}"#.to_vec(),
            "application/json",
        ))
        .mount(&h.gateway)
        .await;

    let resp = h.authed_post("/v1/messages", r#"{"messages":[]}"#).await;
    assert_eq!(resp.status().as_u16(), 200);
    resp.text().await.expect("drain body");

    for _ in 0..200 {
        if h.stats.messages_total.load(Ordering::Relaxed) > 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(h.stats.tokens_in_total.load(Ordering::Relaxed), 21);
    assert_eq!(h.stats.tokens_out_total.load(Ordering::Relaxed), 4);
    assert_eq!(h.stats.messages_total.load(Ordering::Relaxed), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_json_response_off_the_messages_path_is_not_tapped() {
    let h = spawn_harness().await;
    Mock::given(method("POST"))
        .and(path("/v1/complete"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            br#"{"usage":{"input_tokens":21,"output_tokens":4}}"#.to_vec(),
            "application/json",
        ))
        .mount(&h.gateway)
        .await;

    let resp = h.authed_post("/v1/complete", "{}").await;
    resp.text().await.expect("drain body");

    assert_eq!(
        h.stats.messages_total.load(Ordering::Relaxed),
        0,
        "only the messages path feeds the usage counters"
    );
    assert_eq!(h.stats.tokens_in_total.load(Ordering::Relaxed), 0);
    assert_eq!(h.stats.forwarded_total.load(Ordering::Relaxed), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_non_json_messages_response_records_no_tokens() {
    let h = spawn_harness().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(b"plain".to_vec(), "text/plain"))
        .mount(&h.gateway)
        .await;

    let resp = h.authed_post("/v1/messages", "{}").await;
    assert_eq!(resp.text().await.expect("body"), "plain");
    assert_eq!(
        h.stats.messages_total.load(Ordering::Relaxed),
        0,
        "the tap only understands JSON and SSE bodies"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hop_by_hop_headers_are_stripped_and_bridge_headers_injected() {
    let h = spawn_harness().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(b"{}".to_vec(), "application/json"))
        .mount(&h.gateway)
        .await;

    Harness::client()
        .post(h.url("/v1/messages"))
        .header("authorization", format!("Bearer {SECRET}"))
        .header("x-api-key", "client-side-key")
        .header("x-keep-me", "kept")
        .header("content-type", "application/json")
        .body(r#"{"messages":[{"role":"user","content":"hi"}]}"#)
        .send()
        .await
        .expect("request to proxy");

    let requests = h.upstream_requests().await;
    let req = &requests[0];
    assert_eq!(
        bearer_of(req),
        "Bearer upstream-jwt-1",
        "the client's loopback secret is replaced by the gateway JWT"
    );
    assert!(
        req.headers.get("x-api-key").is_none(),
        "x-api-key is hop-by-hop and never reaches the gateway"
    );
    assert_eq!(
        req.headers.get("x-keep-me").and_then(|v| v.to_str().ok()),
        Some("kept"),
        "unrelated client headers are passed through"
    );
    assert_eq!(
        req.headers
            .get("x-systemprompt-bridge")
            .and_then(|v| v.to_str().ok()),
        Some("1")
    );
    assert!(
        req.headers
            .get("x-session-id")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|s| !s.is_empty()),
        "the proxy stamps its session id"
    );
    assert!(
        req.headers
            .get("x-gateway-conversation-id")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|s| !s.is_empty()),
        "a messages body derives a gateway conversation id"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_same_conversation_body_prefix_maps_to_a_stable_conversation_id() {
    let h = spawn_harness().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(b"{}".to_vec(), "application/json"))
        .mount(&h.gateway)
        .await;

    let body = r#"{"messages":[{"role":"user","content":"first turn"}]}"#;
    h.authed_post("/v1/messages", body).await;
    h.authed_post("/v1/messages", body).await;

    let requests = h.upstream_requests().await;
    let ids: Vec<String> = requests
        .iter()
        .map(|r| {
            r.headers
                .get("x-gateway-conversation-id")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    assert_eq!(
        ids[0], ids[1],
        "the same conversation prefix must resolve to one gateway conversation"
    );
    assert!(!ids[0].is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn healthz_is_served_locally_without_a_loopback_secret() {
    let h = spawn_harness().await;

    let get = Harness::client()
        .get(h.url("/healthz"))
        .send()
        .await
        .expect("GET /healthz");
    assert_eq!(get.status().as_u16(), 200);
    assert_eq!(get.text().await.expect("body"), "ok\n");

    let head = Harness::client()
        .head(h.url("/healthz"))
        .send()
        .await
        .expect("HEAD /healthz");
    assert_eq!(head.status().as_u16(), 200);

    assert!(
        h.upstream_requests().await.is_empty(),
        "/healthz never reaches the gateway"
    );
    assert_eq!(
        h.stats.forwarded_total.load(Ordering::Relaxed),
        0,
        "a locally served health check is not a forward"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn otel_posts_are_unauthenticated_and_rewritten_under_v1() {
    let h = spawn_harness().await;
    Mock::given(method("POST"))
        .and(path_regex(r"^/v1/otel.*"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(b"{}".to_vec(), "application/json"))
        .mount(&h.gateway)
        .await;

    let resp = Harness::client()
        .post(h.url("/otel/v1/traces?compression=gzip"))
        .body("payload")
        .send()
        .await
        .expect("otel post");
    assert_eq!(
        resp.status().as_u16(),
        200,
        "no loopback secret is required on the OTLP path"
    );

    let requests = h.upstream_requests().await;
    assert_eq!(requests[0].url.path(), "/v1/otel/v1/traces");
    assert_eq!(
        requests[0].url.query(),
        Some("compression=gzip"),
        "the query string survives the rewrite"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bare_otel_post_is_rewritten_to_v1_otel() {
    let h = spawn_harness().await;
    Mock::given(method("POST"))
        .and(path("/v1/otel"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&h.gateway)
        .await;

    let resp = Harness::client()
        .post(h.url("/otel"))
        .body("payload")
        .send()
        .await
        .expect("otel post");
    assert_eq!(resp.status().as_u16(), 202);
    assert_eq!(h.upstream_requests().await[0].url.path(), "/v1/otel");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unreachable_gateway_yields_502_and_is_recorded() {
    let dead = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("reserve a port");
    let dead_port = dead.local_addr().expect("addr").port();
    drop(dead);

    let h = spawn_with_base(
        MockServer::start().await,
        Some(format!("http://127.0.0.1:{dead_port}")),
    )
    .await;

    let resp = h.authed_post("/v1/messages", "{}").await;
    assert_eq!(resp.status().as_u16(), 502);
    assert_eq!(resp.text().await.expect("body"), "bad gateway\n");
    assert_eq!(h.stats.last_status.load(Ordering::Relaxed), 502);
    assert_eq!(h.stats.forwarded_total.load(Ordering::Relaxed), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_request_that_cannot_mint_a_token_is_reported_as_a_bad_gateway() {
    let gateway = MockServer::start().await;
    let stats = Arc::new(ProxyStats::default());
    let refresh: RefreshFn = Arc::new(|_| Box::pin(async { None }));
    let ctx = ProxyContext {
        runtime_config: shared_runtime_config(&gateway.uri()),
        secret: Arc::new(ProxySecret::new(SECRET)),
        stats: Arc::clone(&stats),
        client: reqwest::Client::new(),
        token_cache: Arc::new(TokenCache::new(refresh)),
        session: Arc::new(SessionContext::new()),
    };
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    tokio::spawn(async move {
        while let Ok((stream, peer)) = listener.accept().await {
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

    let resp = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/v1/messages"))
        .header("authorization", format!("Bearer {SECRET}"))
        .body("{}")
        .send()
        .await
        .expect("request to proxy");
    assert_eq!(resp.status().as_u16(), 502);
    assert!(
        gateway.received_requests().await.expect("requests").is_empty(),
        "no JWT means the request never leaves the machine"
    );
    assert_eq!(stats.last_status.load(Ordering::Relaxed), 502);
}


fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(fut)
}

fn state_sandbox<R>(state: &tempfile::TempDir, f: impl FnOnce() -> R) -> R {
    let root = state.path().display().to_string();
    temp_env::with_vars(
        vec![
            ("XDG_STATE_HOME", Some(root.clone())),
            ("XDG_CACHE_HOME", Some(root.clone())),
            ("XDG_CONFIG_HOME", Some(root.clone())),
            ("HOME", Some(root)),
        ],
        f,
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
fn use_headless_keystore() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let store = linux_keyutils_keyring_store::Store::new().expect("keyutils store");
        keyring_core::set_default_store(store);
    });
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn use_headless_keystore() {}

#[test]
fn a_registered_mcp_server_is_routed_to_with_its_own_headers() {
    let state = tempfile::tempdir().expect("state dir");
    state_sandbox(&state, || {
        block_on(async {
            let upstream = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/mcp"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_raw(br#"{"tools":[]}"#.to_vec(), "application/json"),
                )
                .mount(&upstream)
                .await;

            let meta = state
                .path()
                .join("systemprompt-bridge")
                .join("metadata");
            std::fs::create_dir_all(&meta).expect("metadata dir");
            std::fs::write(
                meta.join("mcp-servers.json"),
                serde_json::json!([{
                    "name": "Salesforce MCP",
                    "url": format!("{}/mcp", upstream.uri()),
                    "transport": "http",
                    "headers": {"x-connector": "salesforce"},
                }])
                .to_string(),
            )
            .expect("mcp fragment");
            systemprompt_bridge::mcp_registry::rehydrate_from_disk();

            let h = spawn_harness().await;
            let resp = h.authed_post("/mcp/salesforce-mcp", "{}").await;
            assert_eq!(resp.status().as_u16(), 200);
            assert_eq!(resp.text().await.expect("body"), r#"{"tools":[]}"#);

            assert!(
                h.upstream_requests().await.is_empty(),
                "an MCP request is routed to the connector, not the gateway"
            );
            let seen = upstream.received_requests().await.expect("requests");
            assert_eq!(
                seen[0].headers.get("x-connector").and_then(|v| v.to_str().ok()),
                Some("salesforce"),
                "the registry's per-server headers are injected"
            );
            assert_eq!(
                bearer_of(&seen[0]),
                "Bearer upstream-jwt-1",
                "the connector is reached with the gateway JWT"
            );
        });
    });
}

#[test]
fn a_hook_route_mints_a_plugin_scoped_token_and_a_401_spares_the_shared_jwt() {
    use_headless_keystore();
    let state = tempfile::tempdir().expect("state dir");
    state_sandbox(&state, || {
        block_on(async {
            let gateway = MockServer::start().await;
            let token_endpoint = format!("{}/v1/oauth/token", gateway.uri());
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/oauth-client"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "client_id": "hook-client",
                    "client_secret": "hook-secret",
                    "scopes": ["hook:govern"],
                    "token_endpoint": token_endpoint,
                })))
                .mount(&gateway)
                .await;
            Mock::given(method("POST"))
                .and(path("/v1/oauth/token"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "access_token": "hook-access-token",
                    "token_type": "Bearer",
                    "expires_in": 3600,
                })))
                .mount(&gateway)
                .await;
            Mock::given(method("POST"))
                .and(path("/api/public/hooks/govern"))
                .respond_with(ResponseTemplate::new(401).set_body_string("stale hook token"))
                .mount(&gateway)
                .await;

            let h = spawn_with_base(gateway, None).await;
            let resp = h
                .authed_post("/api/public/hooks/govern?plugin_id=acme-plugin", "{}")
                .await;
            assert_eq!(resp.status().as_u16(), 401);

            let seen = h.upstream_requests().await;
            let hook_call = seen
                .iter()
                .find(|r| r.url.path() == "/api/public/hooks/govern")
                .expect("the hook route reached the gateway");
            assert_eq!(
                bearer_of(hook_call),
                "Bearer hook-access-token",
                "the hook route carries its plugin-scoped token, not the bridge JWT"
            );
            assert!(
                seen.iter().any(|r| r.url.path() == "/v1/oauth/token"),
                "the plugin token was minted at the client-credentials endpoint"
            );
            assert_eq!(
                h.mints.load(Ordering::Relaxed),
                1,
                "a 401 on a hook route must not invalidate the shared gateway JWT"
            );
        });
    });
}
