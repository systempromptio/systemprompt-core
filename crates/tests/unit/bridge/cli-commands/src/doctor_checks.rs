//! Exercises the gateway-facing doctor checks (`check_mint_jwt`,
//! `check_gateway_reachable`, `check_whoami`, `check_hook_token_mint`) against
//! a wiremock gateway inside a fully sandboxed environment. These checks are
//! async but do not touch the global proxy runtime, so each test builds its
//! own current-thread runtime inside the `temp_env` closure.

use std::path::Path;

use systemprompt_bridge::cli::doctor::auth::{
    check_gateway_reachable, check_hook_token_mint, check_mint_jwt, check_whoami,
};
use systemprompt_bridge::cli::doctor::{Check, Status};
use systemprompt_bridge::config;
use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_bridge::gateway::manifest::ValidatedUrl;
use systemprompt_bridge::gateway::types::HelperOutput;
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
    GatewayClient::new(
        ValidatedUrl::try_new(uri.to_owned()).expect("valid ValidatedUrl"),
        reqwest::Client::new(),
    )
}

fn find<'a>(checks: &'a [Check], name: &str) -> &'a Check {
    checks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no `{name}` check recorded"))
}

#[test]
fn claude_settings_doctor_diagnoses_corrupt_mismatched_and_recoverable_profiles_without_leaking_helper()
 {
    use systemprompt_bridge::cli::doctor::claude_code::check_file;

    sandbox(|root| {
        let settings = root.join("claude-settings.json");
        std::fs::write(&settings, "{ not json").unwrap();
        let corrupt = check_file(&settings, "http://127.0.0.1:8123");
        assert_eq!(corrupt.status, Status::Fail);
        assert!(corrupt.detail.contains("invalid settings JSON"));

        std::fs::write(
            &settings,
            r#"{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:9999"},"apiKeyHelper":"private-helper-value"}"#,
        )
        .unwrap();
        let mismatched = check_file(&settings, "http://127.0.0.1:8123");
        assert_eq!(mismatched.status, Status::Fail);
        assert!(mismatched.detail.contains("routing differs"));
        assert!(!mismatched.detail.contains("private-helper-value"));

        std::fs::write(
            &settings,
            r#"{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:8123"},"apiKeyHelper":"private-helper-value"}"#,
        )
        .unwrap();
        let recovered = check_file(&settings, "http://127.0.0.1:8123");
        assert_eq!(recovered.status, Status::Ok);
        assert!(!recovered.detail.contains("private-helper-value"));
    });
}

#[test]
fn host_profile_secret_doctor_reports_stale_opencode_credentials_then_a_repaired_profile() {
    use systemprompt_bridge::cli::doctor::auth::check_host_profile_secrets;
    use systemprompt_bridge::ids::{HostId, LoopbackSecret};
    use systemprompt_bridge::integration::host_app::ProbeEnv;
    use systemprompt_bridge::proxy::scoped_token::host_token;

    sandbox(|root| {
        let managed = root.join("managed");
        std::fs::create_dir_all(&managed).expect("managed directory");
        let config = root.join("systemprompt/systemprompt-bridge.toml");
        std::fs::create_dir_all(config.parent().expect("config parent")).expect("config directory");
        std::fs::write(
            config,
            format!("[opencode]\nmanaged_dir = '{}'\n", managed.display()),
        )
        .expect("bridge config");
        let profile = managed.join("opencode.json");
        std::fs::write(
            &profile,
            r#"{"provider":{"systemprompt":{"npm":"@ai-sdk/openai-compatible","options":{"baseURL":"http://127.0.0.1:1/v1","headers":{"x-inference-protocol":"openai"}},"models":{"gpt-4.1":{"name":"gpt-4.1"}}}},"model":"systemprompt/gpt-4.1"}"#,
        )
        .expect("stale profile");
        let auth = root.join("opencode/auth.json");
        std::fs::create_dir_all(auth.parent().expect("auth parent")).expect("auth directory");
        let host = HostId::new("opencode");
        let stale_token = host_token(&LoopbackSecret::new("retired-loopback-secret"), &host);
        std::fs::write(
            &auth,
            serde_json::json!({ "systemprompt": { "type": "api", "key": stale_token.as_str() } })
                .to_string(),
        )
        .expect("stale auth token");

        let env = ProbeEnv {
            proxy_port: 48217,
            loopback_secret: Some(LoopbackSecret::new("live-loopback-secret")),
            start_menu: std::sync::Arc::default(),
            expected_managed_servers: None,
            policy_writer_ready: false,
        };
        let stale = check_host_profile_secrets(&env).expect("stale installed profile is diagnosed");
        assert_eq!(stale.status, Status::Fail);
        assert!(stale.detail.contains("OpenCode"), "{}", stale.detail);
        assert!(
            stale.detail.contains("out-of-date loopback secret"),
            "{}",
            stale.detail
        );
        assert!(
            !stale.detail.contains(stale_token.as_str()),
            "{}",
            stale.detail
        );

        let live_token = host_token(env.loopback_secret.as_ref().expect("live secret"), &host);
        std::fs::write(
            &auth,
            serde_json::json!({ "systemprompt": { "type": "api", "key": live_token.as_str() } })
                .to_string(),
        )
        .expect("repair auth token");
        let wrong_port = check_host_profile_secrets(&env).expect("port mismatch is diagnosed");
        assert_eq!(wrong_port.status, Status::Fail);
        assert!(
            wrong_port.detail.contains("OpenCode"),
            "{}",
            wrong_port.detail
        );
        assert!(wrong_port.detail.contains("48217"), "{}", wrong_port.detail);

        let repaired = std::fs::read_to_string(&profile)
            .expect("read stale profile")
            .replace("127.0.0.1:1", "127.0.0.1:48217");
        std::fs::write(&profile, repaired).expect("repair profile");
        let healthy = check_host_profile_secrets(&env).expect("installed profile is reported");
        assert_eq!(healthy.status, Status::Ok, "{}", healthy.detail);
        assert!(healthy.detail.contains("match the live loopback secret"));
    });
}

#[test]
fn mint_jwt_fails_with_a_login_hint_when_no_provider_is_configured() {
    sandbox(|_| {
        let cfg = config::load().expect("valid config");
        let mut checks = Vec::new();
        let bearer = block_on(check_mint_jwt(&cfg, &mut checks, &reqwest::Client::new()));
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
        let cfg = config::load().expect("valid config");
        let mut checks = Vec::new();
        block_on(check_gateway_reachable(
            &cfg,
            &mut checks,
            &reqwest::Client::new(),
        ));
        let check = find(&checks, "gateway reachable");
        assert_eq!(check.status, Status::Ok, "{}", check.detail);
        assert!(check.detail.contains("/health"), "{}", check.detail);

        std::fs::write(&cfg_file, "gateway_url = \"http://127.0.0.1:1\"\n").expect("config");
        let cfg = config::load().expect("valid config");
        let mut checks = Vec::new();
        block_on(check_gateway_reachable(
            &cfg,
            &mut checks,
            &reqwest::Client::new(),
        ));
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
        assert!(
            check.detail.contains("first plugin hook request"),
            "the warning must name where provisioning actually happens; saying it runs on \
             the first sync sends operators to the wrong subsystem: {}",
            check.detail
        );
        assert!(
            !check.detail.contains("first sync after login"),
            "{}",
            check.detail
        );
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
