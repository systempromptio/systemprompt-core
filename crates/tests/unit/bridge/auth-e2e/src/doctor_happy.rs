//! Doctor happy-path coverage: a fully provisioned sandbox (config file, PAT,
//! reachable gateway, loopback secret, pinned pubkey, Cowork session state,
//! org plugin bundle, OAuth client creds) must drive every check through its
//! OK branch.

use std::fs;
use std::path::Path;

use systemprompt_bridge::auth::plugin_oauth::{self, OAuthClientCreds};
use systemprompt_bridge::cli::doctor::{self, Status};
use systemprompt_identifiers::ClientId;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PERSONAL_SESSION_UUID: &str = "00000000-0000-4000-8000-000000000001";

fn use_keyutils_store() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let store = linux_keyutils_keyring_store::Store::new().unwrap();
        keyring_core::set_default_store(store);
    });
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fut)
}

fn sandbox_vars(home: &TempDir) -> Vec<(&'static str, Option<String>)> {
    let root = home.path().to_string_lossy().into_owned();
    vec![
        ("HOME", Some(root.clone())),
        ("XDG_CONFIG_HOME", Some(format!("{root}/.config"))),
        ("XDG_CACHE_HOME", Some(format!("{root}/.cache"))),
        ("XDG_DATA_HOME", Some(format!("{root}/.data"))),
        ("XDG_STATE_HOME", Some(format!("{root}/.state"))),
        ("SP_BRIDGE_PAT", None),
        ("SP_BRIDGE_CONFIG", None),
        ("SP_BRIDGE_GATEWAY_URL", None),
    ]
}

fn write_config(root: &Path, gateway: &str, pat_file: &Path) {
    let dir = root.join(".config").join("systemprompt");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("systemprompt-bridge.toml"),
        format!(
            "gateway_url = \"{gateway}\"\n[pat]\nfile = \"{}\"\n[sync]\npinned_pubkey = \
             \"cGlubmVkLXB1YmtleQ==\"\n",
            pat_file.display()
        ),
    )
    .unwrap();
    fs::write(dir.join("bridge-loopback.key"), "loopback-secret-value").unwrap();
}

fn write_cowork_state(root: &Path) {
    let org = root
        .join(".config")
        .join("Claude-3p")
        .join("local-agent-mode-sessions")
        .join("acct-1")
        .join(PERSONAL_SESSION_UUID);
    fs::create_dir_all(org.join("cowork_plugins")).unwrap();
    fs::write(
        org.join("cowork_settings.json"),
        serde_json::to_vec(&serde_json::json!({
            "enabledPlugins": { "systemprompt-managed@org-provisioned": true }
        }))
        .unwrap(),
    )
    .unwrap();
}

fn write_org_plugin(root: &Path) {
    let plugin = root
        .join(".data")
        .join("Claude")
        .join("org-plugins")
        .join("systemprompt-managed")
        .join(".claude-plugin");
    fs::create_dir_all(&plugin).unwrap();
    fs::write(
        plugin.join("plugin.json"),
        serde_json::to_vec(&serde_json::json!({
            "name": "systemprompt-managed",
            "installationPreference": "required",
        }))
        .unwrap(),
    )
    .unwrap();
}

async fn mount_gateway(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/bridge/pat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "token": "jwt.doctor.token",
            "ttl": 3600,
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/bridge/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "user_id": "user_1",
            "email": "alice@example.com",
        })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "hook.jwt",
            "expires_in": 600,
        })))
        .mount(server)
        .await;
}

fn status_of<'a>(checks: &'a [doctor::Check], name: &str) -> &'a doctor::Check {
    checks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("missing check `{name}`"))
}

#[test]
fn fully_provisioned_sandbox_yields_no_failing_checks() {
    use_keyutils_store();
    let home = TempDir::new().unwrap();
    let root = home.path().to_path_buf();

    let (server, uri) = block_on(async {
        let server = MockServer::start().await;
        mount_gateway(&server).await;
        let uri = server.uri();
        (server, uri)
    });
    let _ = &server;

    let pat_file = root.join("pat.txt");
    fs::write(&pat_file, "sp-live-doctor-pat").unwrap();
    write_config(&root, &uri, &pat_file);
    write_cowork_state(&root);
    write_org_plugin(&root);

    temp_env::with_vars(sandbox_vars(&home), || {
        plugin_oauth::store_creds(&OAuthClientCreds {
            client_id: ClientId::new(format!("doctor-client-{}", std::process::id())),
            client_secret: "doctor-secret".into(),
            token_endpoint: format!("{uri}/oauth/token"),
            scopes: vec!["hook:govern".into(), "hook:track".into()],
        })
        .unwrap();

        let (checks, any_fail) = block_on(doctor::run_checks());

        for check in &checks {
            assert_ne!(
                check.status,
                Status::Fail,
                "check `{}` must not fail: {}",
                check.name,
                check.detail
            );
        }
        assert!(!any_fail);

        for name in [
            "config file",
            "credential source",
            "mint JWT",
            "gateway reachable",
            "authenticated whoami",
            "loopback secret",
            "manifest pubkey pinned",
            "cowork enable",
            "plugin auto-install",
            "personal-session sentinel",
            "hook token mint",
        ] {
            let check = status_of(&checks, name);
            assert_eq!(
                check.status,
                Status::Ok,
                "check `{name}` should be OK: {}",
                check.detail
            );
        }

        plugin_oauth::delete_creds().unwrap();
    });
}

#[test]
fn hook_token_mint_rejection_is_reported_as_failure() {
    use_keyutils_store();
    let home = TempDir::new().unwrap();
    let root = home.path().to_path_buf();

    let (server, uri) = block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(403).set_body_raw("denied", "text/plain"))
            .mount(&server)
            .await;
        let uri = server.uri();
        (server, uri)
    });
    let _ = &server;

    let pat_file = root.join("pat.txt");
    fs::write(&pat_file, "sp-live-doctor-pat").unwrap();
    write_config(&root, &uri, &pat_file);

    temp_env::with_vars(sandbox_vars(&home), || {
        plugin_oauth::store_creds(&OAuthClientCreds {
            client_id: ClientId::new(format!("doctor-reject-{}", std::process::id())),
            client_secret: "doctor-secret".into(),
            token_endpoint: format!("{uri}/oauth/token"),
            scopes: vec![],
        })
        .unwrap();

        let (checks, any_fail) = block_on(doctor::run_checks());
        assert!(any_fail);
        let mint = status_of(&checks, "hook token mint");
        assert_eq!(mint.status, Status::Fail);
        assert!(mint.detail.contains("403"), "detail: {}", mint.detail);

        plugin_oauth::delete_creds().unwrap();
    });
}
