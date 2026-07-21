use super::sandbox::{Sandbox, argv};
use systemprompt_bridge::cli::run_with_args;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Gateway {
    server: MockServer,
    uri: String,
    runtime: tokio::runtime::Runtime,
}

fn gateway() -> Gateway {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let server = runtime.block_on(MockServer::start());
    let uri = server.uri();
    runtime.block_on(async {
        Mock::given(method("POST"))
            .and(path("/v1/auth/bridge/pat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token": "jwt-for-tests",
                "ttl": 900,
                "headers": {},
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/bridge/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "user_id": "user_1",
                "email": "person@example.com",
                "roles": ["member"],
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/auth/bridge/oauth-client"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "client_id": "client-abc",
                "client_secret": "secret-abc",
                "scopes": ["hook:govern", "hook:track"],
                "token_endpoint": format!("{uri}/v1/oauth/token"),
            })))
            .mount(&server)
            .await;
    });
    Gateway {
        server,
        uri,
        runtime,
    }
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

fn authenticated<R>(gw: &Gateway, sb: &Sandbox, f: impl FnOnce() -> R) -> R {
    let mut vars = sb.vars();
    vars.push(("SP_BRIDGE_GATEWAY_URL", Some(gw.uri.clone())));
    vars.push((
        "SP_BRIDGE_PAT",
        Some("sp-live-testprefix.secretsecretsecretsecretsecret012345".to_owned()),
    ));
    temp_env::with_vars(vars, f)
}

#[test]
fn whoami_prints_the_identity_the_gateway_returns() {
    let gw = gateway();
    let sb = Sandbox::new();
    authenticated(&gw, &sb, || {
        let _ = run_with_args(&argv(&["whoami"]));
    });

    let requests = gw
        .runtime
        .block_on(gw.server.received_requests())
        .expect("recorded requests");
    assert!(
        requests
            .iter()
            .any(|r| r.url.path() == "/v1/auth/bridge/pat"),
        "whoami exchanges the PAT first"
    );
    assert!(
        requests.iter().any(|r| {
            r.url.path() == "/v1/bridge/whoami"
                && r.headers
                    .get("authorization")
                    .is_some_and(|v| v == "Bearer jwt-for-tests")
        }),
        "whoami calls the gateway with the freshly minted JWT"
    );
}

#[test]
fn run_emits_the_credential_envelope_for_a_configured_pat() {
    let gw = gateway();
    let sb = Sandbox::new();
    authenticated(&gw, &sb, || {
        let _ = run_with_args(&argv(&["run"]));
        let _ = run_with_args(&argv(&["credential-helper", "--host", "claude-desktop"]));
    });

    let requests = gw
        .runtime
        .block_on(gw.server.received_requests())
        .expect("recorded requests");
    assert!(
        requests
            .iter()
            .any(|r| r.url.path() == "/v1/auth/bridge/pat"),
        "the helper chain reaches the PAT exchange endpoint"
    );
}

#[test]
fn oauth_client_rotate_provisions_and_persists_the_client() {
    let gw = gateway();
    use_headless_keystore();
    let sb = Sandbox::new();
    authenticated(&gw, &sb, || {
        let _ = run_with_args(&argv(&["oauth-client", "rotate"]));
        let creds = systemprompt_bridge::auth::plugin_oauth::load_creds()
            .expect("creds load")
            .expect("rotate persists the provisioned client");
        assert_eq!(creds.client_id.as_str(), "client-abc");
        assert_eq!(creds.scopes.join(" "), "hook:govern hook:track");

        let _ = run_with_args(&argv(&["oauth-client", "status"]));
    });
}

#[test]
fn a_gateway_that_rejects_the_pat_leaves_no_oauth_client_behind() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let server = runtime.block_on(MockServer::start());
    runtime.block_on(async {
        Mock::given(method("POST"))
            .and(path("/v1/auth/bridge/pat"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
    });
    let uri = server.uri();

    use_headless_keystore();
    let sb = Sandbox::new();
    let mut vars = sb.vars();
    vars.push(("SP_BRIDGE_GATEWAY_URL", Some(uri)));
    vars.push((
        "SP_BRIDGE_PAT",
        Some("sp-live-testprefix.secretsecretsecretsecretsecret012345".to_owned()),
    ));
    let creds = temp_env::with_vars(vars, || {
        let _ = run_with_args(&argv(&["oauth-client", "rotate"]));
        let _ = run_with_args(&argv(&["whoami"]));
        let _ = run_with_args(&argv(&["run"]));
        systemprompt_bridge::auth::plugin_oauth::load_creds().expect("creds load")
    });
    assert!(
        creds.is_none(),
        "a rejected PAT must not provision an OAuth client"
    );
}


#[test]
fn sync_through_dispatch_applies_the_manifest_and_writes_the_sentinel() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let server = runtime.block_on(MockServer::start());
    let uri = server.uri();
    runtime.block_on(async {
        Mock::given(method("POST"))
            .and(path("/v1/auth/bridge/pat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token": "jwt-for-sync",
                "ttl": 900,
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/bridge/manifest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "manifest_version": "2026-05-01T12:00:00Z-deadbeef",
                "issued_at": "2026-05-01T12:00:00+00:00",
                "not_before": "2026-05-01T12:00:00+00:00",
                "user_id": "00000000-0000-4000-8000-00000000fee1",
                "user": {
                    "id": "00000000-0000-4000-8000-00000000fee1",
                    "name": "alice",
                    "email": "alice@example.com",
                    "roles": [],
                },
                "plugins": [],
                "skills": [],
                "agents": [],
                "hooks": [],
                "managed_mcp_servers": [],
                "revocations": [],
                "enabled_hosts": [],
                "host_model_protocols": {},
                "artifacts": [],
                "signature": "unused-when-allow-unsigned",
            })))
            .mount(&server)
            .await;
    });

    let sb = Sandbox::new();
    let mut vars = sb.vars();
    vars.push(("SP_BRIDGE_GATEWAY_URL", Some(uri)));
    vars.push((
        "SP_BRIDGE_PAT",
        Some("sp-live-testprefix.secretsecretsecretsecretsecret012345".to_owned()),
    ));
    temp_env::with_vars(vars, || {
        let _ = run_with_args(&argv(&["install"]));
        let _ = run_with_args(&argv(&["sync", "--allow-unsigned", "--force-replay"]));
    });

    let sentinel = sb.metadata().join("last-sync.json");
    let raw = std::fs::read_to_string(&sentinel).expect("sync writes the last-sync sentinel");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("sentinel is JSON");
    assert_eq!(
        parsed["manifest_version"], "2026-05-01T12:00:00Z-deadbeef",
        "the applied manifest version is recorded: {raw}"
    );
    assert_eq!(parsed["user"], "alice@example.com");
    assert!(
        sb.org_plugins().is_dir(),
        "the apply prepares the org-plugins root"
    );
    drop(server);
}

#[test]
fn a_sync_whose_manifest_is_refused_leaves_no_sentinel() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let server = runtime.block_on(MockServer::start());
    let uri = server.uri();
    runtime.block_on(async {
        Mock::given(method("POST"))
            .and(path("/v1/auth/bridge/pat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token": "jwt-for-sync",
                "ttl": 900,
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/bridge/manifest"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
    });

    let sb = Sandbox::new();
    let mut vars = sb.vars();
    vars.push(("SP_BRIDGE_GATEWAY_URL", Some(uri)));
    vars.push((
        "SP_BRIDGE_PAT",
        Some("sp-live-testprefix.secretsecretsecretsecretsecret012345".to_owned()),
    ));
    temp_env::with_vars(vars, || {
        let _ = run_with_args(&argv(&["install"]));
        let _ = run_with_args(&argv(&["sync", "--allow-unsigned", "--force-replay"]));
    });

    assert!(
        !sb.metadata().join("last-sync.json").exists(),
        "a failed manifest fetch must not advance the replay sentinel"
    );
    drop(server);
}
