//! In-process exercise of the CLI command entry points (`cmd_*`).
//!
//! `ExitCode` is opaque (no `PartialEq`, no accessor), so these tests assert
//! observable side effects (config/PAT files created or removed) and that each
//! command runs to completion without panicking inside a fully sandboxed
//! environment. The command bodies drive the context's runtime with
//! `block_on`, so they are invoked directly from the synchronous
//! `temp_env::with_vars` closure (no outer tokio runtime, which would
//! nest-panic).

use systemprompt_bridge::cli::{
    clean, login, logout, oauth_client, status, sync, validate, whoami,
};
use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn ctx() -> std::sync::Arc<BridgeContext> {
    BridgeContext::start(ProxyMode::Attach).expect("runtime builds")
}

fn s(v: &str) -> Option<String> {
    Some(v.to_owned())
}

fn sandbox<R>(gateway: Option<&str>, f: impl FnOnce() -> R) -> R {
    let home = TempDir::new().expect("home tempdir");
    let cfg = TempDir::new().expect("config tempdir");
    let data = TempDir::new().expect("data tempdir");
    let state = TempDir::new().expect("state tempdir");
    if let Some(url) = gateway {
        let dir = cfg.path().join("systemprompt");
        std::fs::create_dir_all(&dir).expect("config dir");
        std::fs::write(
            dir.join("systemprompt-bridge.toml"),
            format!("gateway_url = \"{url}\"\n"),
        )
        .expect("write gateway config");
    }
    let vars: Vec<(&'static str, Option<String>)> = vec![
        ("HOME", s(home.path().to_str().unwrap())),
        ("XDG_CONFIG_HOME", s(cfg.path().to_str().unwrap())),
        ("XDG_DATA_HOME", s(data.path().to_str().unwrap())),
        ("XDG_STATE_HOME", s(state.path().to_str().unwrap())),
        ("SP_BRIDGE_CONFIG", None),
        ("SP_BRIDGE_PAT", None),
    ];
    let result = temp_env::with_vars(vars, f);
    drop((home, cfg, data, state));
    result
}

fn start_gateway() -> (MockServer, String) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/bridge/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "user_id": "user_1",
                "email": "a@e.com",
                "roles": ["member"],
            })))
            .mount(&server)
            .await;
        let uri = server.uri();
        (server, uri)
    })
}

#[test]
fn login_stores_pat_then_logout_and_clean_remove_it() {
    sandbox(None, || {
        let args = vec![
            "systemprompt-bridge".to_owned(),
            "login".to_owned(),
            "sp-live-testprefix.secretsecretsecretsecretsecret012345".to_owned(),
        ];
        let _ = login::cmd_login(&ctx(), &args);
        let cfg_path = systemprompt_bridge::config::config_path().expect("config path resolvable");
        assert!(cfg_path.exists(), "login should create the config file");

        let _ = logout::cmd_logout();
        let _ = clean::cmd_clean();
        // clean wipes the config back to a fresh splash.
        assert!(
            !cfg_path.exists()
                || std::fs::read_to_string(&cfg_path).map_or(true, |c| !c.contains("sp-live")),
            "logout/clean should drop the stored PAT"
        );
    });
}

#[test]
fn login_without_a_terminal_fails_instead_of_waiting_for_a_person() {
    sandbox(None, || {
        let args = vec!["systemprompt-bridge".to_owned(), "login".to_owned()];
        assert_eq!(
            login::cmd_login(&ctx(), &args),
            std::process::ExitCode::from(1),
            "bare `login` starts single sign-on, which cannot complete with no \
             terminal attached; it must report that rather than block on a \
             browser callback or a pasted code that will never arrive"
        );
    });
}

#[test]
fn clean_on_fresh_state_is_ok() {
    sandbox(None, || {
        let _ = clean::cmd_clean();
    });
}

#[test]
fn status_renders_in_sandbox() {
    sandbox(None, || {
        let _ = status::cmd_status();
    });
}

#[test]
fn validate_runs_against_mock_gateway() {
    let (server, uri) = start_gateway();
    sandbox(Some(&uri), || {
        let _ = validate::cmd_validate(&ctx());
    });
    drop(server);
}

#[test]
fn whoami_runs_against_mock_gateway() {
    let (server, uri) = start_gateway();
    sandbox(Some(&uri), || {
        // No credential source in the sandbox, so this exercises the auth-failure
        // path of the wrapper; it must return an ExitCode without panicking.
        let _ = whoami::cmd_whoami(&ctx());
    });
    drop(server);
}

#[test]
fn sync_without_credentials_runs_error_path() {
    let (server, uri) = start_gateway();
    sandbox(Some(&uri), || {
        let args = vec![
            "systemprompt-bridge".to_owned(),
            "sync".to_owned(),
            "--allow-unsigned".to_owned(),
        ];
        let _ = sync::cmd_sync(&ctx(), &args);
    });
    drop(server);
}

#[test]
fn oauth_client_status_and_unknown_subcommand() {
    sandbox(None, || {
        let status_args = vec![
            "systemprompt-bridge".to_owned(),
            "oauth-client".to_owned(),
            "status".to_owned(),
        ];
        let _ = oauth_client::cmd_oauth_client(&ctx(), &status_args);

        let bogus = vec![
            "systemprompt-bridge".to_owned(),
            "oauth-client".to_owned(),
            "no-such-subcommand".to_owned(),
        ];
        let _ = oauth_client::cmd_oauth_client(&ctx(), &bogus);
    });
}
