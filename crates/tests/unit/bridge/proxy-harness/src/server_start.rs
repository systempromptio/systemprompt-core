//! Tests for `proxy::server::start`: real listener bootstrap on an ephemeral
//! port, request serving through the accept loop, and the bind-conflict error.

use std::sync::Arc;

use arc_swap::ArcSwap;
use systemprompt_bridge::config::{RuntimeConfig, SharedRuntimeConfig};
use systemprompt_bridge::proxy::server::start;
use systemprompt_bridge::proxy::session::SessionContext;
use systemprompt_bridge::proxy::token_cache::TokenCache;
use systemprompt_identifiers::ValidatedUrl;

fn runtime_config(uri: &str) -> SharedRuntimeConfig {
    Arc::new(ArcSwap::from_pointee(RuntimeConfig {
        gateway_base: Arc::new(ValidatedUrl::new(uri)),
    }))
}

fn empty_cache() -> Arc<TokenCache> {
    Arc::new(TokenCache::new(Arc::new(|_threshold| {
        Box::pin(async { None })
    })))
}

#[test]
fn start_binds_ephemeral_port_and_serves_requests() {
    let temp = tempfile::tempdir().unwrap();
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        let handle = start(
            &rt,
            0,
            runtime_config("http://127.0.0.1:9"),
            empty_cache(),
            Arc::new(SessionContext::new()),
        )
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
    });
}

#[test]
fn start_falls_back_when_port_already_bound() {
    let temp = tempfile::tempdir().unwrap();
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let blocker = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = blocker.local_addr().unwrap().port();

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        let handle = start(
            &rt,
            port,
            runtime_config("http://127.0.0.1:9"),
            empty_cache(),
            Arc::new(SessionContext::new()),
        )
        .expect("occupied preferred port must fall back to an ephemeral one");
        assert_ne!(
            handle.port, 0,
            "listener must come up despite the v4 conflict"
        );
    });
}
