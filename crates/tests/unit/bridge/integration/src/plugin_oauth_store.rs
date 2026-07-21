//! Tests for plugin OAuth credential storage and minting: on-disk non-secret
//! metadata under `XDG_CACHE_HOME`, the secret in the OS keyring, legacy
//! plaintext-secret migration, and the wiremock-driven provision/mint flows
//! including the 401 rotate-and-retry path.

use std::sync::atomic::{AtomicU32, Ordering};

use systemprompt_bridge::auth::plugin_oauth::{
    self, OAuthClientCreds, mint_or_refresh_plugin_token,
};
use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_identifiers::{ClientId, PluginId, ValidatedUrl};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

static UNIQUE: AtomicU32 = AtomicU32::new(0);

fn use_keyutils_store() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let store = linux_keyutils_keyring_store::Store::new().unwrap();
        keyring_core::set_default_store(store);
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
    use_keyutils_store();
    let temp = tempfile::tempdir().unwrap();
    let out = temp_env::with_var("XDG_CACHE_HOME", Some(temp.path().as_os_str()), body);
    (out, temp)
}

fn creds(client_id: &str) -> OAuthClientCreds {
    OAuthClientCreds {
        client_id: ClientId::new(client_id),
        client_secret: "super-secret".into(),
        token_endpoint: "http://127.0.0.1:1/oauth/token".into(),
        scopes: vec!["hook:govern".into(), "hook:track".into()],
    }
}

#[test]
fn creds_path_is_under_cache_dir() {
    let ((), _temp) = with_cache_home(|| {
        let path = plugin_oauth::creds_path().unwrap();
        assert!(path.ends_with("systemprompt-bridge/oauth_client.json"));
    });
}

#[test]
fn store_then_load_round_trips_via_keyring() {
    let id = unique("client-roundtrip");
    let ((), _temp) = with_cache_home(|| {
        plugin_oauth::store_creds(&creds(&id)).unwrap();

        let path = plugin_oauth::creds_path().unwrap();
        let on_disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(
            on_disk.get("client_secret").is_none(),
            "secret must not be written to disk"
        );

        let loaded = plugin_oauth::load_creds().unwrap().unwrap();
        assert_eq!(loaded.client_id.as_str(), id);
        assert_eq!(loaded.client_secret, "super-secret");
        assert_eq!(loaded.scopes.len(), 2);

        plugin_oauth::delete_creds().unwrap();
        assert!(plugin_oauth::load_creds().unwrap().is_none());
    });
}

#[test]
fn load_creds_none_when_file_missing() {
    let ((), _temp) = with_cache_home(|| {
        assert!(plugin_oauth::load_creds().unwrap().is_none());
    });
}

#[test]
fn legacy_plaintext_secret_is_migrated_into_keyring() {
    let id = unique("client-legacy");
    let ((), _temp) = with_cache_home(|| {
        let path = plugin_oauth::creds_path().unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "client_id": id,
                "client_secret": "legacy-secret",
                "token_endpoint": "http://127.0.0.1:1/oauth/token",
            }))
            .unwrap(),
        )
        .unwrap();

        let loaded = plugin_oauth::load_creds().unwrap().unwrap();
        assert_eq!(loaded.client_secret, "legacy-secret");

        let rewritten: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(
            rewritten.get("client_secret").is_none(),
            "migration must strip the plaintext secret"
        );

        plugin_oauth::delete_creds().unwrap();
    });
}

#[test]
fn metadata_without_keyring_entry_is_unprovisioned() {
    let id = unique("client-nokeyring");
    let ((), _temp) = with_cache_home(|| {
        let path = plugin_oauth::creds_path().unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "client_id": id,
                "token_endpoint": "http://127.0.0.1:1/oauth/token",
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(plugin_oauth::load_creds().unwrap().is_none());
    });
}

#[test]
fn delete_creds_when_missing_is_noop() {
    let ((), _temp) = with_cache_home(|| {
        plugin_oauth::delete_creds().unwrap();
    });
}

fn provision_body(server_uri: &str, client_id: &str, secret: &str) -> serde_json::Value {
    serde_json::json!({
        "client_id": client_id,
        "client_secret": secret,
        "scopes": ["hook:govern", "hook:track"],
        "token_endpoint": format!("{server_uri}/oauth/token"),
    })
}

fn block_on<T>(fut: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fut)
}

#[test]
fn ensure_creds_provisions_once_then_reuses_local_state() {
    let id = unique("client-ensure");
    let ((), _temp) = with_cache_home(|| {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/oauth-client"))
                .respond_with(ResponseTemplate::new(200).set_body_json(provision_body(
                    &server.uri(),
                    &id,
                    "minted-secret",
                )))
                .expect(1)
                .mount(&server)
                .await;

            let client = GatewayClient::new(ValidatedUrl::new(server.uri()));
            let first = plugin_oauth::ensure_creds(&client, "sp-live-pat")
                .await
                .unwrap();
            assert_eq!(first.client_secret, "minted-secret");

            let second = plugin_oauth::ensure_creds(&client, "sp-live-pat")
                .await
                .unwrap();
            assert_eq!(
                second.client_id.as_str(),
                id,
                "second call must reuse local state"
            );

            plugin_oauth::delete_creds().unwrap();
        });
    });
}

#[test]
fn refresh_creds_always_reprovisions() {
    let id = unique("client-refresh");
    let ((), _temp) = with_cache_home(|| {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/oauth-client"))
                .respond_with(ResponseTemplate::new(200).set_body_json(provision_body(
                    &server.uri(),
                    &id,
                    "rotated",
                )))
                .expect(1)
                .mount(&server)
                .await;

            let client = GatewayClient::new(ValidatedUrl::new(server.uri()));
            let out = plugin_oauth::refresh_creds(&client, "sp-live-pat")
                .await
                .unwrap();
            assert_eq!(out.client_secret, "rotated");
            plugin_oauth::delete_creds().unwrap();
        });
    });
}

struct MintOnceRotate {
    calls: AtomicU32,
}

impl Respond for MintOnceRotate {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            ResponseTemplate::new(401).set_body_raw("stale client", "text/plain")
        } else {
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "hook.jwt.rotated",
                "expires_in": 900,
            }))
        }
    }
}

#[test]
fn mint_or_refresh_rotates_client_on_401_and_retries() {
    let id = unique("client-mint401");
    let plugin = unique("plugin-mint401");
    let ((), _temp) = with_cache_home(|| {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/oauth-client"))
                .respond_with(ResponseTemplate::new(200).set_body_json(provision_body(
                    &server.uri(),
                    &id,
                    "s1",
                )))
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path("/oauth/token"))
                .respond_with(MintOnceRotate {
                    calls: AtomicU32::new(0),
                })
                .mount(&server)
                .await;

            let client = GatewayClient::new(ValidatedUrl::new(server.uri()));
            let token =
                mint_or_refresh_plugin_token(&client, "sp-live-pat", &PluginId::new(&plugin))
                    .await
                    .unwrap();
            assert_eq!(token.access_token, "hook.jwt.rotated");

            let cached =
                mint_or_refresh_plugin_token(&client, "sp-live-pat", &PluginId::new(&plugin))
                    .await
                    .unwrap();
            assert_eq!(
                cached.access_token, "hook.jwt.rotated",
                "second call must come from the fresh-token cache"
            );

            plugin_oauth::delete_creds().unwrap();
        });
    });
}

#[test]
fn mint_or_refresh_success_path_caches_token() {
    let id = unique("client-mintok");
    let plugin = unique("plugin-mintok");
    let ((), _temp) = with_cache_home(|| {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/oauth-client"))
                .respond_with(ResponseTemplate::new(200).set_body_json(provision_body(
                    &server.uri(),
                    &id,
                    "s1",
                )))
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path("/oauth/token"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "access_token": "hook.jwt.ok",
                    "expires_in": 900,
                })))
                .expect(1)
                .mount(&server)
                .await;

            let client = GatewayClient::new(ValidatedUrl::new(server.uri()));
            let first =
                mint_or_refresh_plugin_token(&client, "sp-live-pat", &PluginId::new(&plugin))
                    .await
                    .unwrap();
            let second =
                mint_or_refresh_plugin_token(&client, "sp-live-pat", &PluginId::new(&plugin))
                    .await
                    .unwrap();
            assert_eq!(first.access_token, "hook.jwt.ok");
            assert_eq!(second.access_token, "hook.jwt.ok");

            plugin_oauth::delete_creds().unwrap();
        });
    });
}
