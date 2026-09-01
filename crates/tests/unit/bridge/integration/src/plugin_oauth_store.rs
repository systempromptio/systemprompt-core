//! Tests for plugin OAuth credential storage and minting: on-disk non-secret
//! metadata under `XDG_CACHE_HOME`, the secret in the OS keyring, legacy
//! plaintext-secret migration, and the wiremock-driven provision/mint flows
//! including the 401 rotate-and-retry path.

use std::sync::atomic::{AtomicU32, Ordering};

use systemprompt_bridge::auth::plugin_oauth::{
    self, OAuthClientCreds, mint_or_refresh_plugin_token,
};
use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_bridge::ids::BearerToken;
use systemprompt_identifiers::{ClientId, PluginId, ValidatedUrl};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

static UNIQUE: AtomicU32 = AtomicU32::new(0);

// The bridge's own lazy platform-store bootstrap must stand down before the
// first entry is created, so a headless store is installed here instead. Linux
// CI gets the kernel keyring, which needs no Secret Service daemon; elsewhere
// the in-memory mock plays the same role without reaching for the platform
// keychain, which would prompt or fail on a developer's machine.
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

fn creds(client_id: &str) -> OAuthClientCreds {
    OAuthClientCreds {
        client_id: ClientId::new(client_id),
        client_secret: "super-secret".into(),
        token_endpoint: "http://127.0.0.1:1/oauth/token".into(),
        scopes: vec!["hook:govern".into(), "hook:track".into()],
        gateway: Some("http://127.0.0.1:1".into()),
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

            let client =
                GatewayClient::new(ValidatedUrl::new(server.uri()), reqwest::Client::new());
            let first = plugin_oauth::ensure_creds(&client, &BearerToken::new("bridge-jwt"))
                .await
                .unwrap();
            assert_eq!(first.client_secret, "minted-secret");

            let second = plugin_oauth::ensure_creds(&client, &BearerToken::new("bridge-jwt"))
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

            let client =
                GatewayClient::new(ValidatedUrl::new(server.uri()), reqwest::Client::new());
            let out = plugin_oauth::refresh_creds(&client, &BearerToken::new("bridge-jwt"))
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

            let client =
                GatewayClient::new(ValidatedUrl::new(server.uri()), reqwest::Client::new());
            let token = mint_or_refresh_plugin_token(
                &client,
                &BearerToken::new("bridge-jwt"),
                &PluginId::new(&plugin),
            )
            .await
            .unwrap();
            assert_eq!(token.access_token, "hook.jwt.rotated");

            let cached = mint_or_refresh_plugin_token(
                &client,
                &BearerToken::new("bridge-jwt"),
                &PluginId::new(&plugin),
            )
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

            let client =
                GatewayClient::new(ValidatedUrl::new(server.uri()), reqwest::Client::new());
            let first = mint_or_refresh_plugin_token(
                &client,
                &BearerToken::new("bridge-jwt"),
                &PluginId::new(&plugin),
            )
            .await
            .unwrap();
            let second = mint_or_refresh_plugin_token(
                &client,
                &BearerToken::new("bridge-jwt"),
                &PluginId::new(&plugin),
            )
            .await
            .unwrap();
            assert_eq!(first.access_token, "hook.jwt.ok");
            assert_eq!(second.access_token, "hook.jwt.ok");

            plugin_oauth::delete_creds().unwrap();
        });
    });
}

// Why: an OAuth client is registered with one gateway, and the hook tokens it
// mints are signed by that gateway's authority. Reusing a production-registered
// client while the bridge points at a local server made it mint from
// production's token endpoint and present the result to the local governance
// webhook, which rejected it as an unknown signing key and blocked every tool
// call. Nothing expired, so it never recovered on its own.
#[test]
fn a_client_registered_with_one_gateway_is_not_reused_for_another() {
    let first_id = unique("client-gw-a");
    let second_id = unique("client-gw-b");
    let ((), _temp) = with_cache_home(|| {
        block_on(async {
            let gateway_a = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/oauth-client"))
                .respond_with(ResponseTemplate::new(200).set_body_json(provision_body(
                    &gateway_a.uri(),
                    &first_id,
                    "secret-a",
                )))
                .expect(1)
                .mount(&gateway_a)
                .await;

            let gateway_b = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/oauth-client"))
                .respond_with(ResponseTemplate::new(200).set_body_json(provision_body(
                    &gateway_b.uri(),
                    &second_id,
                    "secret-b",
                )))
                .expect(1)
                .mount(&gateway_b)
                .await;

            let client_a =
                GatewayClient::new(ValidatedUrl::new(gateway_a.uri()), reqwest::Client::new());
            let a = plugin_oauth::ensure_creds(&client_a, &BearerToken::new("bridge-jwt"))
                .await
                .unwrap();
            assert_eq!(a.gateway.as_deref(), Some(gateway_a.uri().as_str()));

            let client_b =
                GatewayClient::new(ValidatedUrl::new(gateway_b.uri()), reqwest::Client::new());
            let b = plugin_oauth::ensure_creds(&client_b, &BearerToken::new("bridge-jwt"))
                .await
                .unwrap();

            assert_eq!(
                b.client_secret, "secret-b",
                "the second gateway must register its own client, not inherit the first's"
            );
            assert_eq!(
                b.token_endpoint,
                format!("{}/oauth/token", gateway_b.uri()),
                "minting must go to the gateway the bridge is actually pointed at"
            );
            assert_eq!(b.gateway.as_deref(), Some(gateway_b.uri().as_str()));

            plugin_oauth::delete_creds().unwrap();
        });
    });
}

// Why: a file written before the client became gateway-aware records no
// gateway. Unknown is not "matches" — it is exactly the state that caused the
// bug — so it re-provisions once rather than being trusted.
#[test]
fn a_stored_client_with_no_recorded_gateway_is_reprovisioned() {
    let id = unique("client-legacy-gw");
    let ((), _temp) = with_cache_home(|| {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/oauth-client"))
                .respond_with(ResponseTemplate::new(200).set_body_json(provision_body(
                    &server.uri(),
                    &id,
                    "reprovisioned",
                )))
                .expect(1)
                .mount(&server)
                .await;

            let mut stale = creds(&unique("client-stale"));
            stale.gateway = None;
            plugin_oauth::store_creds(&stale).unwrap();

            let client =
                GatewayClient::new(ValidatedUrl::new(server.uri()), reqwest::Client::new());
            let out = plugin_oauth::ensure_creds(&client, &BearerToken::new("bridge-jwt"))
                .await
                .unwrap();

            assert_eq!(out.client_secret, "reprovisioned");
            assert_eq!(out.gateway.as_deref(), Some(server.uri().as_str()));

            plugin_oauth::delete_creds().unwrap();
        });
    });
}
