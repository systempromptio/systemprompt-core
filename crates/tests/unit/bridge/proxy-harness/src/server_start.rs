//! Tests for `proxy::server::start`: real listener bootstrap on an ephemeral
//! port, request serving through the accept loop, and the bind-conflict error.

use std::sync::Arc;

use arc_swap::ArcSwap;
use systemprompt_bridge::config::{RuntimeConfig, SharedRuntimeConfig};
use systemprompt_bridge::proxy::secret;
use systemprompt_bridge::proxy::server::{ServerParts, start};
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

fn parts(uri: &str) -> ServerParts {
    ServerParts {
        loopback: secret::proxy_init().expect("the sandbox mints a loopback secret"),
        runtime_config: runtime_config(uri),
        token_cache: empty_cache(),
        session: Arc::new(SessionContext::new()),
        deps: systemprompt_bridge::proxy::ProxyDeps {
            install_id: systemprompt_bridge::proxy::identity::InstallId::establish(),
            mcp_registry: systemprompt_bridge::mcp_registry::empty_slot(),
            activity: systemprompt_bridge::activity::ActivityLog::new(),
            http: reqwest::Client::new(),
        },
    }
}

#[test]
fn start_binds_serves_and_survives_an_occupied_port() {
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

        // An occupied v4 port still comes up, over v6 on the same port.
        let blocker = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let taken = blocker.local_addr().unwrap().port();
        let second = start(rt.handle(), taken, parts("http://127.0.0.1:9"))
            .expect("occupied preferred port must still yield a listener");
        assert_ne!(
            second.port, 0,
            "listener must come up despite the v4 conflict"
        );
    });
}
