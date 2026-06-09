//! In-process exercise of the CLI command entry points (`cmd_*`).
//!
//! `ExitCode` is opaque (no `PartialEq`, no accessor), so these tests assert
//! observable side effects (config/PAT files created or removed) and that each
//! command runs to completion without panicking inside a fully sandboxed
//! environment. The command bodies internally drive `proxy::block_on`, so they
//! are invoked directly from the synchronous `temp_env::with_vars` closure (no
//! outer tokio runtime, which would nest-panic).

use systemprompt_bridge::cli::{
    clean, login, logout, oauth_client, status, sync, validate, whoami,
};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn s(v: &str) -> Option<String> {
    Some(v.to_owned())
}

fn sandbox<R>(extra: Vec<(&'static str, Option<String>)>, f: impl FnOnce() -> R) -> R {
    let home = TempDir::new().expect("home tempdir");
    let cfg = TempDir::new().expect("config tempdir");
    let data = TempDir::new().expect("data tempdir");
    let state = TempDir::new().expect("state tempdir");
    let mut vars: Vec<(&'static str, Option<String>)> = vec![
        ("HOME", s(home.path().to_str().unwrap())),
        ("XDG_CONFIG_HOME", s(cfg.path().to_str().unwrap())),
        ("XDG_DATA_HOME", s(data.path().to_str().unwrap())),
        ("XDG_STATE_HOME", s(state.path().to_str().unwrap())),
        ("SP_BRIDGE_CONFIG", None),
        ("SP_BRIDGE_PAT", None),
        ("SP_BRIDGE_GATEWAY_URL", None),
    ];
    vars.extend(extra);
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
    sandbox(vec![], || {
        let args = vec![
            "systemprompt-bridge".to_owned(),
            "login".to_owned(),
            "sp-live-testprefix.secretsecretsecretsecretsecret012345".to_owned(),
        ];
        let _ = login::cmd_login(&args);
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
fn login_without_token_is_usage_error() {
    sandbox(vec![], || {
        let args = vec!["systemprompt-bridge".to_owned(), "login".to_owned()];
        // Missing token: exercises the usage-error branch (ExitCode 64). Just
        // ensure it runs without panicking.
        let _ = login::cmd_login(&args);
    });
}

#[test]
fn clean_on_fresh_state_is_ok() {
    sandbox(vec![], || {
        let _ = clean::cmd_clean();
    });
}

#[test]
fn status_renders_in_sandbox() {
    sandbox(vec![], || {
        let _ = status::cmd_status();
    });
}

#[test]
fn validate_runs_against_mock_gateway() {
    let (server, uri) = start_gateway();
    sandbox(vec![("SP_BRIDGE_GATEWAY_URL", Some(uri))], || {
        let _ = validate::cmd_validate();
    });
    drop(server);
}

#[test]
fn whoami_runs_against_mock_gateway() {
    let (server, uri) = start_gateway();
    sandbox(vec![("SP_BRIDGE_GATEWAY_URL", Some(uri))], || {
        // No credential source in the sandbox, so this exercises the auth-failure
        // path of the wrapper; it must return an ExitCode without panicking.
        let _ = whoami::cmd_whoami();
    });
    drop(server);
}

#[test]
fn sync_without_credentials_runs_error_path() {
    let (server, uri) = start_gateway();
    sandbox(vec![("SP_BRIDGE_GATEWAY_URL", Some(uri))], || {
        let args = vec![
            "systemprompt-bridge".to_owned(),
            "sync".to_owned(),
            "--allow-unsigned".to_owned(),
        ];
        let _ = sync::cmd_sync(&args);
    });
    drop(server);
}

#[test]
fn oauth_client_status_and_unknown_subcommand() {
    sandbox(vec![], || {
        let status_args = vec![
            "systemprompt-bridge".to_owned(),
            "oauth-client".to_owned(),
            "status".to_owned(),
        ];
        let _ = oauth_client::cmd_oauth_client(&status_args);

        let bogus = vec![
            "systemprompt-bridge".to_owned(),
            "oauth-client".to_owned(),
            "no-such-subcommand".to_owned(),
        ];
        let _ = oauth_client::cmd_oauth_client(&bogus);
    });
}
