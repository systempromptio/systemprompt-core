//! A stored OAuth client is reused only for the gateway it was registered
//! with, compared by identity: a trailing slash or a different default-port
//! spelling of the same gateway is the same gateway and does not force a
//! re-provision.

use std::sync::atomic::{AtomicU32, Ordering};

use systemprompt_bridge::auth::plugin_oauth::{self, OAuthClientCreds};
use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_bridge::ids::BearerToken;
use systemprompt_identifiers::{ClientId, ValidatedUrl};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

static UNIQUE: AtomicU32 = AtomicU32::new(0);

#[cfg(target_os = "linux")]
fn use_headless_keystore() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        keyring_core::set_default_store(
            linux_keyutils_keyring_store::Store::new().expect("keyutils store"),
        );
    });
}

#[cfg(not(target_os = "linux"))]
fn use_headless_keystore() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        keyring_core::set_default_store(keyring_core::mock::Store::new().expect("mock store"));
    });
}

fn unique(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        UNIQUE.fetch_add(1, Ordering::Relaxed)
    )
}

fn with_cache_home<T>(body: impl FnOnce() -> T) -> (T, TempDir) {
    use_headless_keystore();
    let temp = tempfile::tempdir().unwrap();
    let out = temp_env::with_var("XDG_CACHE_HOME", Some(temp.path().as_os_str()), body);
    (out, temp)
}

fn block_on<T>(fut: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fut)
}

#[test]
fn a_stored_gateway_with_a_trailing_slash_is_the_same_gateway() {
    let id = unique("client-slash");
    let ((), _temp) = with_cache_home(|| {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/oauth-client"))
                .respond_with(ResponseTemplate::new(500))
                .expect(0)
                .mount(&server)
                .await;

            let stored = OAuthClientCreds {
                client_id: ClientId::new(&id),
                client_secret: "stored-secret".into(),
                token_endpoint: format!("{}/oauth/token", server.uri()),
                scopes: vec!["hook:govern".into()],
                gateway: Some(format!("{}/", server.uri())),
            };
            plugin_oauth::store_creds(&stored).unwrap();

            let client = GatewayClient::new(
                ValidatedUrl::try_new(server.uri()).expect("valid gateway url"),
                reqwest::Client::new(),
            );
            let out = plugin_oauth::ensure_creds(&client, &BearerToken::new("bridge-jwt"))
                .await
                .expect("the stored client is reused without contacting the gateway");
            assert_eq!(out.client_id, ClientId::new(&id));
            assert_eq!(out.client_secret, "stored-secret");

            plugin_oauth::delete_creds().unwrap();
        });
    });
}

#[test]
fn a_stored_gateway_that_cannot_be_parsed_is_an_error_not_a_silent_reprovision() {
    let id = unique("client-badgw");
    let ((), _temp) = with_cache_home(|| {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/oauth-client"))
                .respond_with(ResponseTemplate::new(500))
                .expect(0)
                .mount(&server)
                .await;

            let stored = OAuthClientCreds {
                client_id: ClientId::new(&id),
                client_secret: "stored-secret".into(),
                token_endpoint: format!("{}/oauth/token", server.uri()),
                scopes: vec![],
                gateway: Some("ftp://not-a-gateway".into()),
            };
            plugin_oauth::store_creds(&stored).unwrap();

            let client = GatewayClient::new(
                ValidatedUrl::try_new(server.uri()).expect("valid gateway url"),
                reqwest::Client::new(),
            );
            let err = plugin_oauth::ensure_creds(&client, &BearerToken::new("bridge-jwt"))
                .await
                .expect_err("a malformed recorded gateway is not the absence of one");
            assert!(err.to_string().to_lowercase().contains("gateway"), "{err}");

            plugin_oauth::delete_creds().unwrap();
        });
    });
}
