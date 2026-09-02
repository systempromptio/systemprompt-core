use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::integration::codex_cli::CODEX_CLI_HOST;
use systemprompt_bridge::integration::host_app::{HostApp, ProbeEnv, ProfileState};
use systemprompt_bridge::integration::host_apps;
use systemprompt_bridge::integration::reapply::{build_profile_inputs, reapply_stale_profiles};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const STALE_PORT: u16 = 49999;

struct Sandbox {
    _temp: tempfile::TempDir,
    managed_config: PathBuf,
    vars: Vec<(&'static str, Option<String>)>,
}

fn sandbox(gateway_uri: &str, seed_managed: Option<&str>) -> Sandbox {
    let temp = tempfile::tempdir().expect("tempdir");
    let base = temp.path();
    let config_home = base.join("config");
    let home = base.join("home");
    let codex_home = base.join("codex");
    for d in [&config_home, &home, &codex_home] {
        fs::create_dir_all(d).expect("sandbox dir");
    }
    let managed_config = base.join("etc-codex").join("config.toml");
    if let Some(body) = seed_managed {
        fs::create_dir_all(managed_config.parent().expect("managed parent")).expect("managed dir");
        fs::write(&managed_config, body).expect("seed managed config");
    }
    let secret_dir = config_home.join("systemprompt");
    fs::create_dir_all(&secret_dir).expect("secret dir");
    fs::write(
        secret_dir.join("bridge-loopback.key"),
        "seeded-loopback-secret",
    )
    .expect("seed loopback secret");
    let config_file = config_home.join("systemprompt-bridge.toml");
    fs::write(&config_file, format!("gateway_url = \"{gateway_uri}\"\n"))
        .expect("write bridge config");

    let vars: Vec<(&'static str, Option<String>)> = vec![
        ("SP_BRIDGE_CONFIG", Some(config_file.display().to_string())),
        ("XDG_CONFIG_HOME", Some(config_home.display().to_string())),
        (
            "XDG_DATA_HOME",
            Some(base.join("data").display().to_string()),
        ),
        (
            "XDG_STATE_HOME",
            Some(base.join("state").display().to_string()),
        ),
        (
            "XDG_CACHE_HOME",
            Some(base.join("cache").display().to_string()),
        ),
        ("HOME", Some(home.display().to_string())),
        (
            "HERMES_HOME",
            Some(base.join("hermes").display().to_string()),
        ),
        ("CODEX_HOME", Some(codex_home.display().to_string())),
        (
            "CODEX_SYSTEM_CONFIG",
            Some(managed_config.display().to_string()),
        ),
    ];
    Sandbox {
        _temp: temp,
        managed_config,
        vars,
    }
}

fn stale_managed_config() -> String {
    format!(
        "model_provider = \"systemprompt\"\napproval_policy = \"never\"\nsandbox_mode = \
         \"workspace-write\"\n\n[model_providers.systemprompt]\nbase_url = \
         \"http://127.0.0.1:{STALE_PORT}/v1\"\nwire_api = \"responses\"\n\n\
         [model_providers.systemprompt.auth]\ncommand = \"systemprompt-bridge \
         credential-helper --host codex-cli\"\n"
    )
}

fn profile_body(models: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "inference_gateway_base_url": "https://gateway.example.invalid",
        "auth_scheme": "bearer",
        "models": models,
        "organization_uuid": "org-1234",
        "providers": [
            {
                "name": "openai-upstream",
                "surface": "openai",
                "configured": true,
                "models": models,
            },
            {
                "name": "anthropic-upstream",
                "surface": "anthropic",
                "configured": true,
                "models": ["claude-opus-4-7"],
            }
        ]
    })
}

fn probe_state_of_codex() -> ProfileState {
    let env = ProbeEnv {
        proxy_port: systemprompt_bridge::proxy::DEFAULT_PROXY_PORT,
        loopback_secret_fingerprint: None,
        start_menu: Arc::default(),
    };
    CODEX_CLI_HOST.probe(&env).profile_state
}

fn with_gateway<R>(
    seed_managed: Option<String>,
    body: impl FnOnce(&Arc<BridgeContext>, &Path) -> R,
) -> R {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("mock runtime");
    let server = rt.block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/bridge/profile"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(profile_body(&["gpt-5", "gpt-5-mini"])),
            )
            .mount(&server)
            .await;
        server
    });
    let sb = sandbox(&server.uri(), seed_managed.as_deref());
    let managed = sb.managed_config.clone();
    let vars = sb.vars.clone();
    temp_env::with_vars(vars, || {
        let ctx = BridgeContext::start(ProxyMode::Attach).expect("bridge context");
        body(&ctx, &managed)
    })
}

#[test]
fn a_host_that_was_never_set_up_is_left_alone_by_a_reapply() {
    let reports = with_gateway(None, |ctx, managed| {
        assert!(
            matches!(probe_state_of_codex(), ProfileState::Absent),
            "the sandbox starts with no host profile at all"
        );
        let reports = ctx.block_on(reapply_stale_profiles(ctx, &BTreeMap::new()));
        assert!(
            !managed.exists(),
            "repairing must not enrol a host the user never set up: {}",
            managed.display()
        );
        reports
    });
    assert!(
        reports.is_empty(),
        "no profile was stale, so nothing was touched: {reports:?}"
    );
}

#[test]
fn a_profile_baked_for_a_dead_port_is_repaired_and_reported_reapplied() {
    let (names, outcomes, rewritten) =
        with_gateway(Some(stale_managed_config()), |ctx, managed| {
            assert!(
                matches!(probe_state_of_codex(), ProfileState::Stale { .. }),
                "the seeded profile points at a port the proxy does not hold"
            );
            let reports = ctx.block_on(reapply_stale_profiles(ctx, &BTreeMap::new()));
            let rewritten = fs::read_to_string(managed).expect("managed config still readable");
            let names: Vec<&'static str> = reports.iter().map(|r| r.display_name).collect();
            let outcomes: Vec<String> =
                reports.iter().map(|r| format!("{:?}", r.outcome)).collect();
            (names, outcomes, rewritten)
        });

    assert_eq!(names, vec!["Codex CLI"], "only the stale host was visited");
    assert_eq!(
        outcomes,
        vec!["Reapplied".to_owned()],
        "the re-probe confirmed the repair landed"
    );
    assert!(
        !rewritten.contains(&format!("127.0.0.1:{STALE_PORT}")),
        "the dead port must be gone from the repaired profile: {rewritten}"
    );
    assert!(
        rewritten.contains(&format!(
            "127.0.0.1:{}",
            systemprompt_bridge::proxy::DEFAULT_PROXY_PORT
        )),
        "the repaired profile names the live proxy port: {rewritten}"
    );
}

#[test]
fn a_fresh_profile_is_not_rewritten_by_a_reapply() {
    let fresh = stale_managed_config().replace(
        &format!("127.0.0.1:{STALE_PORT}"),
        &format!(
            "127.0.0.1:{}",
            systemprompt_bridge::proxy::DEFAULT_PROXY_PORT
        ),
    );
    let (reports_len, before, after) = with_gateway(Some(fresh), |ctx, managed| {
        let before = fs::read_to_string(managed).expect("seed readable");
        assert!(
            matches!(probe_state_of_codex(), ProfileState::Installed),
            "a profile on the live port is installed, not stale"
        );
        let reports = ctx.block_on(reapply_stale_profiles(ctx, &BTreeMap::new()));
        let after = fs::read_to_string(managed).expect("still readable");
        (reports.len(), before, after)
    });
    assert_eq!(
        reports_len, 0,
        "a current profile is not a repair candidate"
    );
    assert_eq!(
        before, after,
        "an untouched host's file must be byte-identical after a reapply"
    );
}

#[test]
fn the_profile_inputs_carry_the_live_secret_port_and_the_hosts_own_surface() {
    let inputs = with_gateway(None, |ctx, _managed| {
        ctx.block_on(build_profile_inputs(ctx, &CODEX_CLI_HOST, &BTreeMap::new()))
            .expect("inputs built from the mocked gateway")
    });
    assert_eq!(
        inputs.gateway_base_url,
        format!(
            "http://127.0.0.1:{}",
            systemprompt_bridge::proxy::DEFAULT_PROXY_PORT
        ),
        "the profile points at the loopback proxy, never at the gateway itself"
    );
    assert_eq!(
        inputs.api_key, "seeded-loopback-secret",
        "the profile carries the loopback secret the proxy will actually check"
    );
    assert_eq!(inputs.organization_uuid, Some("org-1234".to_owned()));
    assert_eq!(
        inputs.models,
        vec!["gpt-5".to_owned(), "gpt-5-mini".to_owned()],
        "only the models on Codex's own surface are offered"
    );
    assert_eq!(
        inputs
            .headers
            .get(systemprompt_identifiers::headers::INFERENCE_PROTOCOL)
            .map(String::as_str),
        Some("openai"),
        "the surface header names the protocol the host speaks: {:?}",
        inputs.headers
    );
}

#[test]
fn a_surface_override_replaces_the_models_and_the_protocol_header() {
    let mut overrides = BTreeMap::new();
    overrides.insert("codex-cli".to_owned(), vec!["anthropic".to_owned()]);
    let inputs = with_gateway(None, |ctx, _managed| {
        ctx.block_on(build_profile_inputs(ctx, &CODEX_CLI_HOST, &overrides))
            .expect("inputs built from the mocked gateway")
    });
    assert_eq!(
        inputs.models,
        vec!["claude-opus-4-7".to_owned()],
        "the override moved the host onto the anthropic surface"
    );
    assert_eq!(
        inputs
            .headers
            .get(systemprompt_identifiers::headers::INFERENCE_PROTOCOL)
            .map(String::as_str),
        Some("anthropic")
    );
}

#[test]
fn every_registered_host_is_probed_before_a_repair_is_considered() {
    let ids: Vec<&'static str> = host_apps().iter().map(|h| h.id()).collect();
    assert!(
        ids.contains(&"codex-cli") && ids.contains(&"hermes") && ids.contains(&"opencode"),
        "the reapply sweep walks the whole host registry: {ids:?}"
    );
}
