//! Tests for `proxy::server::start`: real listener bootstrap on an ephemeral
//! port, request serving through the accept loop, and the bind-conflict error.

use std::sync::Arc;

use arc_swap::ArcSwap;
use systemprompt_bridge::config::{RuntimeConfig, SharedRuntimeConfig};
use systemprompt_bridge::gateway::types::HelperOutput;
use systemprompt_bridge::ids::BearerToken;
use systemprompt_bridge::proxy::secret;
use systemprompt_bridge::proxy::server::{ServerParts, start};
use systemprompt_bridge::proxy::session::SessionContext;
use systemprompt_bridge::proxy::token_cache::TokenCache;
use systemprompt_identifiers::ValidatedUrl;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn runtime_config(uri: &str) -> SharedRuntimeConfig {
    Arc::new(ArcSwap::from_pointee(RuntimeConfig {
        gateway_base: Arc::new(ValidatedUrl::try_new(uri).expect("valid ValidatedUrl")),
    }))
}

fn empty_cache() -> Arc<TokenCache> {
    Arc::new(TokenCache::new(Arc::new(|_threshold| {
        Box::pin(async {
            Err(systemprompt_bridge::proxy::forward::ForwardError::Auth(
                "no credential provider produced a token".into(),
            ))
        })
    })))
}

fn parts(uri: &str) -> ServerParts {
    ServerParts {
        loopback: secret::proxy_init().expect("the sandbox mints a loopback secret"),
        runtime_config: runtime_config(uri),
        token_cache: empty_cache(),
        session: Arc::new(SessionContext::new()),
        deps: systemprompt_bridge::proxy::ProxyDeps {
            install_id: systemprompt_bridge::proxy::identity::InstallId::establish()
                .expect("the sandbox mints an install id"),
            mcp_registry: systemprompt_bridge::mcp_registry::empty_slot(),
            activity: systemprompt_bridge::activity::ActivityLog::new(),
            http: reqwest::Client::new(),
            plugin_tokens: Arc::new(
                systemprompt_bridge::auth::plugin_oauth::PluginTokenCache::default(),
            ),
        },
    }
}

fn authenticated_parts(uri: &str) -> ServerParts {
    let mut parts = parts(uri);
    parts.token_cache = Arc::new(TokenCache::new(Arc::new(|_threshold| {
        Box::pin(async {
            Ok(HelperOutput {
                token: BearerToken::new("upstream-jwt"),
                ttl: 3600,
                headers: Default::default(),
            })
        })
    })));
    parts
}

fn hanging_gateway(rt: &tokio::runtime::Runtime) -> (MockServer, std::sync::mpsc::Receiver<()>) {
    let (accepted_tx, accepted_rx) = std::sync::mpsc::sync_channel(1);
    let server = rt.block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(move |_: &wiremock::Request| {
                let _ = accepted_tx.try_send(());
                ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_secs(30))
                    .set_body_json(serde_json::json!({"data": []}))
            })
            .mount(&server)
            .await;
        server
    });
    (server, accepted_rx)
}

#[test]
fn start_binds_serves_and_refuses_an_occupied_port() {
    let temp = tempfile::tempdir().unwrap();
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        let handle = start(rt.handle(), 0, parts("http://127.0.0.1:9"))
            .expect("proxy must start on an ephemeral port");
        assert_ne!(handle.port, 0);

        let status = rt.block_on(async {
            reqwest::Client::new()
                .get(format!("http://127.0.0.1:{}/v1/models", handle.port))
                .send()
                .await
                .unwrap()
                .status()
        });
        assert!(
            status.is_client_error(),
            "unauthenticated request must be rejected, got {status}"
        );

        assert!(
            temp.path()
                .join("systemprompt")
                .join("bridge-loopback.key")
                .is_file(),
            "start must mint the loopback secret"
        );

        // An occupied advertised port is a startup failure, never a silent
        // bind on another address family the client was not told about.
        let blocker = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let taken = blocker.local_addr().unwrap().port();
        let error = start(rt.handle(), taken, parts("http://127.0.0.1:9"))
            .err()
            .expect("an occupied advertised port must refuse to start");
        assert_eq!(
            error.kind(),
            std::io::ErrorKind::AddrInUse,
            "the failure must name the occupied port: {error}"
        );
    });
}

#[test]
fn owned_server_drain_releases_its_port_and_refuses_new_requests() {
    let temp = tempfile::tempdir().unwrap();
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let served = start(rt.handle(), 0, parts("http://127.0.0.1:9")).unwrap();
        let port = served.port;
        assert!(
            served.drain(std::time::Duration::from_secs(2)),
            "the owned listener drains before its deadline"
        );
        assert!(
            std::net::TcpListener::bind(("127.0.0.1", port)).is_ok(),
            "draining drops the listener so a replacement bridge can bind the advertised port"
        );
        let result = rt.block_on(async {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(200))
                .build()
                .expect("HTTP client builds")
                .get(format!("http://127.0.0.1:{port}/v1/models"))
                .send()
                .await
        });
        assert!(
            result.is_err(),
            "a drained server accepts no new HTTP request"
        );
    });
}

#[test]
fn drain_timeout_still_releases_the_listener_and_finishes_after_the_client_leaves() {
    let temp = tempfile::tempdir().unwrap();
    temp_env::with_vars(
        [
            ("XDG_CONFIG_HOME", Some(temp.path().as_os_str())),
            ("SP_BRIDGE_PAT", Some(std::ffi::OsStr::new("sp-live-a.b"))),
        ],
        || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap();
            let (gateway, accepted) = hanging_gateway(&rt);
            let served = start(rt.handle(), 0, authenticated_parts(&gateway.uri())).unwrap();
            let port = served.port;
            let secret =
                std::fs::read_to_string(temp.path().join("systemprompt/bridge-loopback.key"))
                    .unwrap();
            let request = rt.spawn(async move {
                reqwest::Client::new()
                    .get(format!("http://127.0.0.1:{port}/v1/models"))
                    .bearer_auth(secret.trim())
                    .send()
                    .await
            });
            accepted
                .recv_timeout(std::time::Duration::from_secs(2))
                .expect("the authenticated request reached the delayed upstream");

            assert!(
                !served.drain(std::time::Duration::from_millis(75)),
                "an open keep-alive connection outlives the caller's short deadline"
            );
            assert!(
                std::net::TcpListener::bind(("127.0.0.1", port)).is_ok(),
                "the listener is released before in-flight connections finish"
            );

            request.abort();
            assert!(
                served.drain(std::time::Duration::from_secs(2)),
                "once the in-flight client is cancelled, the already-started drain completes"
            );
        },
    );
}

#[test]
fn hard_drain_deadline_aborts_a_stuck_keep_alive_and_records_the_reason() {
    let temp = tempfile::tempdir().unwrap();
    temp_env::with_vars(
        [
            ("XDG_CONFIG_HOME", Some(temp.path().as_os_str())),
            ("SP_BRIDGE_PAT", Some(std::ffi::OsStr::new("sp-live-a.b"))),
        ],
        || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap();
            let activity = systemprompt_bridge::activity::ActivityLog::new();
            let (gateway, accepted) = hanging_gateway(&rt);
            let mut server_parts = authenticated_parts(&gateway.uri());
            server_parts.deps.activity = activity.clone();
            let served = start(rt.handle(), 0, server_parts).unwrap();
            let port = served.port;
            let secret =
                std::fs::read_to_string(temp.path().join("systemprompt/bridge-loopback.key"))
                    .unwrap();
            let request = rt.spawn(async move {
                reqwest::Client::new()
                    .get(format!("http://127.0.0.1:{port}/v1/models"))
                    .bearer_auth(secret.trim())
                    .send()
                    .await
            });
            accepted
                .recv_timeout(std::time::Duration::from_secs(2))
                .expect("the authenticated request reached the delayed upstream");

            assert!(
                !served.drain(std::time::Duration::from_secs(6)),
                "the server reports that its hard five-second deadline expired"
            );
            assert!(
                activity.snapshot_recent(10).iter().any(|entry| {
                    entry.level == systemprompt_bridge::activity::LogLevel::Error
                        && entry.line == "proxy drain: connections were still open at the deadline"
                }),
                "the activity feed explains why shutdown was forced"
            );
            assert!(
                std::net::TcpListener::bind(("127.0.0.1", port)).is_ok(),
                "forced drain still releases the advertised port"
            );
            request.abort();
        },
    );
}

#[test]
fn public_proxy_stats_follow_forwarding_failures_and_reset_with_a_restarted_owner() {
    let temp = tempfile::tempdir().unwrap();
    let config_dir = temp.path().join("systemprompt");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("systemprompt-bridge.toml");
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let gateway = rt.block_on(MockServer::start());
    std::fs::write(
        &config_path,
        format!("gateway_url = \"{}\"\n", gateway.uri()),
    )
    .unwrap();

    temp_env::with_vars(
        [
            ("XDG_CONFIG_HOME", Some(temp.path().as_os_str().to_owned())),
            ("XDG_STATE_HOME", Some(temp.path().as_os_str().to_owned())),
            ("SP_BRIDGE_CONFIG", Some(config_path.as_os_str().to_owned())),
            (
                "SP_BRIDGE_PAT",
                Some(std::ffi::OsString::from("sp-live-a.b")),
            ),
        ],
        || {
            rt.block_on(async {
                Mock::given(method("POST"))
                    .and(path("/v1/auth/bridge/pat"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "token": "jwt.owned.proxy",
                        "ttl": 900,
                        "headers": {}
                    })))
                    .expect(1)
                    .mount(&gateway)
                    .await;
                Mock::given(method("POST"))
                    .and(path("/v1/messages"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "usage": {"input_tokens": 11, "output_tokens": 7}
                    })))
                    .expect(1)
                    .mount(&gateway)
                    .await;
            });
            let mut faults = Vec::new();
            let proxy = systemprompt_bridge::proxy::ProxyHandle::serve(
                rt.handle(),
                parts(&gateway.uri()).deps,
                &mut faults,
            );
            assert!(faults.is_empty(), "startup faults: {faults:?}");
            assert!(proxy.is_serving());
            let secret = std::fs::read_to_string(config_dir.join("bridge-loopback.key")).unwrap();
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .unwrap();
            let response = rt.block_on(async {
                client
                    .post(format!("http://127.0.0.1:{}/v1/messages", proxy.port()))
                    .bearer_auth(secret.trim())
                    .json(&serde_json::json!({"messages": [{"role": "user", "content": "hi"}]}))
                    .send()
                    .await
                    .unwrap()
            });
            assert_eq!(response.status().as_u16(), 200);
            rt.block_on(response.bytes()).unwrap();

            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while serde_json::to_value(
                systemprompt_bridge::wire::payloads::ProxyStatsPayload::current(&proxy),
            )
            .unwrap()["messages_total"]
                != 1
            {
                assert!(
                    std::time::Instant::now() < deadline,
                    "usage was not published"
                );
                std::thread::yield_now();
            }

            let success = serde_json::to_value(
                systemprompt_bridge::wire::payloads::ProxyStatsPayload::current(&proxy),
            )
            .unwrap();
            assert_eq!(success["forwarded_total"], 1);
            assert_eq!(success["messages_total"], 1);
            assert_eq!(success["tokens_in_total"], 11);
            assert_eq!(success["tokens_out_total"], 7);
            assert_eq!(success["last_status"], 200);
            assert!(success["last_latency_ms"].as_u64().is_some());
            assert!(success["last_forwarded_at_unix"].as_u64().unwrap() > 0);

            rt.block_on(async {
                gateway.reset().await;
                Mock::given(method("POST"))
                    .and(path("/v1/messages"))
                    .respond_with(ResponseTemplate::new(502).set_body_string("owned failure"))
                    .expect(1)
                    .mount(&gateway)
                    .await;
            });
            let failed = rt.block_on(async {
                client
                    .post(format!("http://127.0.0.1:{}/v1/messages", proxy.port()))
                    .bearer_auth(secret.trim())
                    .json(&serde_json::json!({"messages": [{"role": "user", "content": "again"}]}))
                    .send()
                    .await
                    .unwrap()
            });
            assert_eq!(failed.status().as_u16(), 502);
            rt.block_on(failed.bytes()).unwrap();
            let after_failure = serde_json::to_value(
                systemprompt_bridge::wire::payloads::ProxyStatsPayload::current(&proxy),
            )
            .unwrap();
            assert_eq!(after_failure["forwarded_total"], 2);
            assert_eq!(after_failure["messages_total"], 1);
            assert_eq!(after_failure["tokens_in_total"], 11);
            assert_eq!(after_failure["tokens_out_total"], 7);
            assert_eq!(after_failure["last_status"], 502);
            assert!(after_failure["last_latency_ms"].as_u64().is_some());
            assert!(after_failure["last_forwarded_at_unix"].as_u64().unwrap() > 0);

            drop(client);
            assert!(
                proxy
                    .served()
                    .unwrap()
                    .drain(std::time::Duration::from_secs(2))
            );
            proxy.forget_recorded_port().unwrap();
            drop(proxy);

            let attached = systemprompt_bridge::proxy::ProxyHandle::attach(
                parts(&gateway.uri()).deps,
                &mut faults,
            );
            assert!(!attached.is_serving());
            assert_eq!(
                serde_json::to_value(
                    systemprompt_bridge::wire::payloads::ProxyStatsPayload::current(&attached),
                )
                .unwrap(),
                serde_json::json!({
                    "forwarded_total": 0,
                    "messages_total": 0,
                    "tokens_in_total": 0,
                    "tokens_out_total": 0,
                    "last_status": 0,
                    "last_latency_ms": 0,
                    "last_forwarded_at_unix": 0
                })
            );
            drop(attached);

            let restarted = systemprompt_bridge::proxy::ProxyHandle::serve(
                rt.handle(),
                parts(&gateway.uri()).deps,
                &mut faults,
            );
            assert!(faults.is_empty(), "lifecycle faults: {faults:?}");
            assert!(restarted.is_serving());
            assert_eq!(
                serde_json::to_value(
                    systemprompt_bridge::wire::payloads::ProxyStatsPayload::current(&restarted),
                )
                .unwrap(),
                serde_json::json!({
                    "forwarded_total": 0,
                    "messages_total": 0,
                    "tokens_in_total": 0,
                    "tokens_out_total": 0,
                    "last_status": 0,
                    "last_latency_ms": 0,
                    "last_forwarded_at_unix": 0
                })
            );
            assert!(
                restarted
                    .served()
                    .unwrap()
                    .drain(std::time::Duration::from_secs(2))
            );
            restarted.forget_recorded_port().unwrap();
        },
    );
}
