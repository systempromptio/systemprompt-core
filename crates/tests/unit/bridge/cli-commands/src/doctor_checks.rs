//! Exercises the gateway-facing doctor checks (`check_mint_jwt`,
//! `check_gateway_reachable`, `check_whoami`, `check_hook_token_mint`) against
//! a wiremock gateway inside a fully sandboxed environment. These checks are
//! async but do not touch the global proxy runtime, so each test builds its
//! own current-thread runtime inside the `temp_env` closure.

use std::path::Path;

use systemprompt_bridge::auth::types::HelperOutput;
use systemprompt_bridge::cli::doctor::auth::{
    check_gateway_reachable, check_hook_token_mint, check_mint_jwt, check_whoami,
};
use systemprompt_bridge::cli::doctor::{Check, Status};
use systemprompt_bridge::config;
use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_bridge::gateway::manifest::ValidatedUrl;
use systemprompt_bridge::ids::BearerToken;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sandbox<R>(f: impl FnOnce(&Path) -> R) -> R {
    let dir = TempDir::new().expect("sandbox dir");
    let root = dir.path().display().to_string();
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(root.clone())),
        ("XDG_CONFIG_HOME", Some(root.clone())),
        ("XDG_CACHE_HOME", Some(root.clone())),
        ("XDG_DATA_HOME", Some(root.clone())),
        ("XDG_STATE_HOME", Some(root)),
        ("SP_BRIDGE_CONFIG", None),
        ("SP_BRIDGE_PAT", None),
        ("SP_BRIDGE_GATEWAY_URL", None),
    ];
    let path = dir.path().to_path_buf();
    temp_env::with_vars(vars, || f(&path))
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(f)
}

fn start_mock() -> (MockServer, tokio::runtime::Runtime) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let server = rt.block_on(MockServer::start());
    (server, rt)
}

fn bearer() -> HelperOutput {
    HelperOutput {
        token: BearerToken::new("test-bearer"),
        ttl: 3600,
        headers: std::collections::HashMap::new(),
    }
}

fn client_for(uri: &str) -> GatewayClient {
    GatewayClient::new(ValidatedUrl::new(uri.to_owned()))
}

fn find<'a>(checks: &'a [Check], name: &str) -> &'a Check {
    checks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no `{name}` check recorded"))
}

#[test]
fn mint_jwt_fails_with_a_login_hint_when_no_provider_is_configured() {
    sandbox(|_| {
        let cfg = config::load();
        let mut checks = Vec::new();
        let bearer = block_on(check_mint_jwt(&cfg, &mut checks));
        assert!(bearer.is_none(), "no provider can mint a bearer");
        let check = find(&checks, "mint JWT");
        assert_eq!(check.status, Status::Fail, "{}", check.detail);
        assert!(check.detail.contains("login"), "{}", check.detail);
    });
}

#[test]
fn gateway_reachable_passes_on_health_200_and_fails_on_a_closed_port() {
    let (server, rt) = start_mock();
    rt.block_on(
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server),
    );
    sandbox(|root| {
        let cfg_file = root.join("systemprompt").join("systemprompt-bridge.toml");
        std::fs::create_dir_all(cfg_file.parent().unwrap()).expect("config dir");
        std::fs::write(&cfg_file, format!("gateway_url = \"{}\"\n", server.uri())).expect("config");
        let cfg = config::load();
        let mut checks = Vec::new();
        block_on(check_gateway_reachable(&cfg, &mut checks));
        let check = find(&checks, "gateway reachable");
        assert_eq!(check.status, Status::Ok, "{}", check.detail);
        assert!(check.detail.contains("/health"), "{}", check.detail);

        std::fs::write(&cfg_file, "gateway_url = \"http://127.0.0.1:1\"\n").expect("config");
        let cfg = config::load();
        let mut checks = Vec::new();
        block_on(check_gateway_reachable(&cfg, &mut checks));
        let check = find(&checks, "gateway reachable");
        assert_eq!(check.status, Status::Fail, "{}", check.detail);
        assert!(check.detail.contains("127.0.0.1:1"), "{}", check.detail);
    });
}

#[test]
fn whoami_check_reports_a_skip_when_no_bearer_was_minted() {
    let client = client_for("http://127.0.0.1:1");
    let mut checks = Vec::new();
    block_on(check_whoami(&client, None, &mut checks));
    let check = find(&checks, "authenticated whoami");
    assert_eq!(check.status, Status::Fail, "{}", check.detail);
    assert!(check.detail.contains("skipped"), "{}", check.detail);
}

#[test]
fn whoami_check_maps_a_401_to_the_revoked_pat_diagnosis() {
    let (server, rt) = start_mock();
    rt.block_on(
        Mock::given(method("GET"))
            .and(path("/v1/bridge/whoami"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server),
    );
    let client = client_for(&server.uri());
    let mut checks = Vec::new();
    block_on(check_whoami(&client, Some(&bearer()), &mut checks));
    let check = find(&checks, "authenticated whoami");
    assert_eq!(check.status, Status::Fail, "{}", check.detail);
    assert!(
        check.detail.contains("invalid or revoked"),
        "{}",
        check.detail
    );
    assert!(check.detail.contains("login"), "{}", check.detail);
}

#[test]
fn whoami_check_passes_on_identity_and_fails_generically_on_a_500() {
    let (server, rt) = start_mock();
    rt.block_on(
        Mock::given(method("GET"))
            .and(path("/v1/bridge/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "user_id": "user_abc",
                "email": "e@example.com",
                "roles": ["member"]
            })))
            .mount(&server),
    );
    let client = client_for(&server.uri());
    let mut checks = Vec::new();
    block_on(check_whoami(&client, Some(&bearer()), &mut checks));
    let ok = find(&checks, "authenticated whoami");
    assert_eq!(ok.status, Status::Ok, "{}", ok.detail);

    let (err_server, rt2) = start_mock();
    rt2.block_on(
        Mock::given(method("GET"))
            .and(path("/v1/bridge/whoami"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&err_server),
    );
    let client = client_for(&err_server.uri());
    let mut checks = Vec::new();
    block_on(check_whoami(&client, Some(&bearer()), &mut checks));
    let fail = find(&checks, "authenticated whoami");
    assert_eq!(fail.status, Status::Fail, "{}", fail.detail);
    assert!(fail.detail.contains("whoami failed"), "{}", fail.detail);
}

#[test]
fn hook_token_check_warns_when_no_oauth_client_is_provisioned() {
    sandbox(|_| {
        let client = client_for("http://127.0.0.1:1");
        let check = block_on(check_hook_token_mint(&client));
        assert_eq!(check.status, Status::Warn, "{}", check.detail);
        assert!(check.detail.contains("first sync"), "{}", check.detail);
    });
}

#[test]
fn hook_token_check_fails_when_the_stored_creds_are_unreadable() {
    sandbox(|root| {
        let creds = root.join("systemprompt-bridge").join("oauth_client.json");
        std::fs::create_dir_all(creds.parent().unwrap()).expect("cache dir");
        std::fs::write(&creds, "not json at all").expect("garbage creds");
        let client = client_for("http://127.0.0.1:1");
        let check = block_on(check_hook_token_mint(&client));
        assert_eq!(check.status, Status::Fail, "{}", check.detail);
        assert!(
            check.detail.contains("load OAuth client creds"),
            "{}",
            check.detail
        );
    });
}
