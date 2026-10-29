use std::collections::BTreeMap;
use systemprompt_bridge::ids::HostToken;
use systemprompt_bridge::integration::codex::CODEX_CLI_HOST;
use systemprompt_bridge::integration::host_app::{
    HostApp, ProfileGenInputs, ProfileState, StaleReason,
};
use tempfile::TempDir;

use crate::support::probe_env;

fn codex_sandbox<R>(managed_toml: Option<&str>, f: impl FnOnce() -> R) -> R {
    let home = TempDir::new().expect("codex home");
    let system = home.path().join("managed_config.toml");
    if let Some(body) = managed_toml {
        std::fs::write(&system, body).expect("seed codex managed config");
    }
    let vars: Vec<(&str, Option<String>)> = vec![
        ("CODEX_HOME", Some(home.path().display().to_string())),
        ("CODEX_SYSTEM_CONFIG", Some(system.display().to_string())),
    ];
    let out = temp_env::with_vars(vars, f);
    drop(home);
    out
}

const COMPLETE: &str = r#"
model_provider = "systemprompt"
approval_policy = "never"
sandbox_mode = "workspace-write"

[model_providers.systemprompt]
base_url = "http://127.0.0.1:48217/v1"
wire_api = "responses"

[model_providers.systemprompt.auth]
command = "systemprompt-bridge credential-helper --host codex-cli"
refresh_interval_ms = 60000

[model_providers.systemprompt.http_headers]
"x-tenant" = "acme-corp"

[otel]
log_user_prompt = false

[analytics]
enabled = true
"#;

#[test]
fn an_absent_codex_config_probes_as_absent() {
    let snapshot = codex_sandbox(None, || CODEX_CLI_HOST.probe(&probe_env()));
    assert_eq!(
        snapshot.host_id,
        systemprompt_models::bridge::host::HostKind::CodexCli
    );
    assert!(
        matches!(snapshot.profile_state, ProfileState::Absent),
        "no config file means an absent profile, got {:?}",
        snapshot.profile_state
    );
    assert!(snapshot.profile_source.is_none());
    assert!(snapshot.profile_keys.is_empty());
}

#[test]
fn a_hand_written_codex_config_is_stale_on_the_settings_the_writer_would_change() {
    let snapshot = codex_sandbox(Some(COMPLETE), || CODEX_CLI_HOST.probe(&probe_env()));
    match &snapshot.profile_state {
        ProfileState::Stale { reason, keys } => {
            assert_eq!(*reason, StaleReason::Settings, "{keys:?}");
            assert!(
                keys.contains(&"model_providers.systemprompt.auth.command".to_owned())
                    && keys.contains(&"analytics.enabled".to_owned()),
                "the helper path and the analytics switch differ from what the writer sets: \
                 {keys:?}"
            );
            assert!(
                !keys.contains(&"model_providers.systemprompt.base_url".to_owned()),
                "the endpoint matches the running proxy: {keys:?}"
            );
        },
        other => panic!("expected Stale, got {other:?}"),
    }
    assert_eq!(
        snapshot
            .profile_keys
            .get("model_providers.systemprompt.base_url")
            .map(String::as_str),
        Some("http://127.0.0.1:48217/v1")
    );
    assert_eq!(
        snapshot
            .profile_keys
            .get("model_providers.systemprompt.auth.refresh_interval_ms")
            .map(String::as_str),
        Some("60000"),
        "integers are stringified"
    );
    assert_eq!(
        snapshot
            .profile_keys
            .get("otel.log_user_prompt")
            .map(String::as_str),
        Some("false"),
        "booleans are stringified"
    );
    assert_eq!(
        snapshot
            .profile_keys
            .get("model_providers.systemprompt.http_headers.x-tenant")
            .map(String::as_str),
        Some("<present, 9 chars>"),
        "the tenant header is redacted to a length"
    );
    assert!(
        snapshot
            .profile_source
            .as_deref()
            .is_some_and(|s| s.ends_with("managed_config.toml")),
        "the probe reports which file it read"
    );
}

#[test]
fn a_codex_config_without_the_endpoint_is_partial() {
    let snapshot = codex_sandbox(
        Some("model_provider = \"systemprompt\"\n\n[analytics]\nenabled = true\n"),
        || CODEX_CLI_HOST.probe(&probe_env()),
    );
    match snapshot.profile_state {
        ProfileState::Partial { missing_required } => {
            assert!(
                missing_required.contains(&"model_providers.systemprompt.base_url".to_owned())
                    && missing_required
                        .contains(&"model_providers.systemprompt.auth.command".to_owned()),
                "missing keys are reported: {missing_required:?}"
            );
        },
        other => panic!("expected Partial, got {other:?}"),
    }
}

#[test]
fn a_malformed_codex_config_is_unverifiable_not_absent() {
    let snapshot = codex_sandbox(Some("this is [not toml"), || {
        CODEX_CLI_HOST.probe(&probe_env())
    });
    assert!(
        matches!(snapshot.profile_state, ProfileState::Unverifiable { .. }),
        "a TOML parse failure is reported, never read as an absent profile: {:?}",
        snapshot.profile_state
    );
    assert!(snapshot.probe_error.is_some());
}

#[test]
fn a_provider_block_only_in_the_users_own_config_is_not_governance() {
    let home = TempDir::new().expect("codex home");
    std::fs::write(home.path().join("config.toml"), COMPLETE).expect("user config");
    let managed = home.path().join("absent-managed.toml");
    let vars: Vec<(&str, Option<String>)> = vec![
        ("CODEX_HOME", Some(home.path().display().to_string())),
        ("CODEX_SYSTEM_CONFIG", Some(managed.display().to_string())),
    ];
    let snapshot = temp_env::with_vars(vars, || CODEX_CLI_HOST.probe(&probe_env()));
    assert!(
        matches!(snapshot.profile_state, ProfileState::Absent),
        "a block the user can edit must not read as installed: {:?}",
        snapshot.profile_state
    );
    assert!(snapshot.profile_keys.is_empty());
}

fn inputs() -> ProfileGenInputs {
    let mut headers = BTreeMap::new();
    headers.insert("x-inference-protocol".to_owned(), "responses".to_owned());
    ProfileGenInputs {
        model_limits: Default::default(),
        model_names: Default::default(),
        gateway_base_url: "http://127.0.0.1:48217".to_owned(),
        host_token: HostToken::new("loopback-secret-value"),
        models: vec!["gpt-5".to_owned()],
        default_model: None,
        organization_uuid: Some("00000000-0000-4000-8000-000000000009".to_owned()),
        headers,
        mcp_servers: Some(Vec::new()),
    }
}

#[test]
fn generating_a_profile_writes_a_config_carrying_the_loopback_endpoint() {
    let generated = codex_sandbox(None, || {
        CODEX_CLI_HOST
            .generate_profile(&inputs())
            .expect("profile generated")
    });
    let body = std::fs::read_to_string(&generated.path).expect("generated profile readable");
    assert!(body.contains("http://127.0.0.1:48217"), "{body}");
    assert!(
        body.contains("model_provider = \"systemprompt\""),
        "the generated profile selects the bridge provider: {body}"
    );
    assert!(
        body.contains("credential-helper"),
        "codex is pointed at the bridge credential helper: {body}"
    );
    assert_eq!(generated.bytes, body.len());
    assert_ne!(generated.payload_uuid, generated.profile_uuid);
    _ = std::fs::remove_file(&generated.path);
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn installing_a_profile_merges_it_into_the_managed_config() {
    let home = TempDir::new().expect("codex home");
    let managed = home.path().join("etc").join("config.toml");
    std::fs::create_dir_all(managed.parent().expect("parent")).expect("etc dir");
    std::fs::write(&managed, "user_key = \"kept\"\n").expect("existing managed config");

    let vars: Vec<(&str, Option<String>)> = vec![
        ("CODEX_HOME", Some(home.path().display().to_string())),
        ("CODEX_SYSTEM_CONFIG", Some(managed.display().to_string())),
    ];
    temp_env::with_vars(vars, || {
        let generated = CODEX_CLI_HOST
            .generate_profile(&inputs())
            .expect("profile generated");
        CODEX_CLI_HOST
            .install_profile(
                &generated.path,
                &systemprompt_bridge::elevation_gate::ElevationGate::new(),
            )
            .expect("install merges into the managed config");
        _ = std::fs::remove_file(&generated.path);
    });

    let merged = std::fs::read_to_string(&managed).expect("merged config");
    assert!(
        merged.contains("user_key = \"kept\""),
        "user-authored keys survive the merge: {merged}"
    );
    assert!(
        merged.contains("http://127.0.0.1:48217"),
        "the bridge block is merged in: {merged}"
    );
}


#[test]
fn the_codex_host_describes_itself_as_a_toml_cli_tool() {
    use systemprompt_bridge::integration::host_app::{ConfigFormat, HostAppKind};

    assert_eq!(CODEX_CLI_HOST.display_name(), "Codex CLI");
    assert_eq!(CODEX_CLI_HOST.icon_id(), "codex-cli");
    assert_eq!(CODEX_CLI_HOST.kind(), HostAppKind::CliTool);
    assert_eq!(CODEX_CLI_HOST.config_format(), ConfigFormat::Toml);
    assert!(
        CODEX_CLI_HOST.download_url().starts_with("https://"),
        "the download URL is offered: {}",
        CODEX_CLI_HOST.download_url()
    );
    assert!(
        CODEX_CLI_HOST
            .description()
            .contains("managed configuration"),
        "{}",
        CODEX_CLI_HOST.description()
    );
    assert_eq!(
        systemprompt_bridge::gateway::model_view::host_client(CODEX_CLI_HOST.id()),
        systemprompt_models::providers::ClientFamily::OpenAi,
        "Codex lists the models the catalog advertises to openai clients"
    );
    assert!(
        !CODEX_CLI_HOST.install_action_label().is_empty(),
        "the install action is labelled for the current platform"
    );
}
