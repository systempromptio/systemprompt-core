use std::collections::BTreeMap;
use systemprompt_bridge::integration::codex_cli::CODEX_CLI_HOST;
use systemprompt_bridge::integration::host_app::{HostApp, ProfileGenInputs, ProfileState};
use tempfile::TempDir;

fn codex_sandbox<R>(config_toml: Option<&str>, f: impl FnOnce() -> R) -> R {
    let home = TempDir::new().expect("codex home");
    if let Some(body) = config_toml {
        std::fs::write(home.path().join("config.toml"), body).expect("seed codex config");
    }
    let system = home.path().join("no-managed-config.toml");
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
    let snapshot = codex_sandbox(None, || CODEX_CLI_HOST.probe());
    assert_eq!(snapshot.host_id, "codex-cli");
    assert!(
        matches!(snapshot.profile_state, ProfileState::Absent),
        "no config file means an absent profile, got {:?}",
        snapshot.profile_state
    );
    assert!(snapshot.profile_source.is_none());
    assert!(snapshot.profile_keys.is_empty());
}

#[test]
fn a_complete_codex_config_probes_as_installed_with_a_redacted_tenant() {
    let snapshot = codex_sandbox(Some(COMPLETE), || CODEX_CLI_HOST.probe());
    assert!(
        matches!(snapshot.profile_state, ProfileState::Installed),
        "every required key is present, got {:?}",
        snapshot.profile_state
    );
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
            .is_some_and(|s| s.ends_with("config.toml")),
        "the probe reports which file it read"
    );
}

#[test]
fn a_partial_codex_config_lists_the_missing_required_keys() {
    let snapshot = codex_sandbox(
        Some("model_provider = \"systemprompt\"\n\n[analytics]\nenabled = true\n"),
        || CODEX_CLI_HOST.probe(),
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
fn a_malformed_codex_config_falls_back_to_an_empty_read() {
    let snapshot = codex_sandbox(Some("this is [not toml"), || CODEX_CLI_HOST.probe());
    assert!(
        matches!(snapshot.profile_state, ProfileState::Absent),
        "a TOML parse failure degrades to Absent, not a panic"
    );
    assert!(snapshot.profile_source.is_none());
}

#[test]
fn the_managed_config_wins_over_the_user_config() {
    let home = TempDir::new().expect("codex home");
    std::fs::write(
        home.path().join("config.toml"),
        "model_provider = \"user-choice\"\n",
    )
    .expect("user config");
    let managed = home.path().join("managed.toml");
    std::fs::write(&managed, "model_provider = \"managed-choice\"\n").expect("managed config");

    let vars: Vec<(&str, Option<String>)> = vec![
        ("CODEX_HOME", Some(home.path().display().to_string())),
        ("CODEX_SYSTEM_CONFIG", Some(managed.display().to_string())),
    ];
    let snapshot = temp_env::with_vars(vars, || CODEX_CLI_HOST.probe());
    assert_eq!(
        snapshot.profile_keys.get("model_provider").map(String::as_str),
        Some("managed-choice"),
        "the managed scope takes precedence"
    );
}

fn inputs() -> ProfileGenInputs {
    let mut headers = BTreeMap::new();
    headers.insert("x-inference-protocol".to_owned(), "responses".to_owned());
    ProfileGenInputs {
        gateway_base_url: "http://127.0.0.1:48217".to_owned(),
        api_key: "loopback-secret-value".to_owned(),
        models: vec!["gpt-5".to_owned()],
        organization_uuid: Some("00000000-0000-4000-8000-000000000009".to_owned()),
        headers,
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
            .install_profile(&generated.path)
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
    use systemprompt_bridge::integration::host_app::{ConfigFormat, HostKind};

    assert_eq!(CODEX_CLI_HOST.display_name(), "Codex CLI");
    assert_eq!(CODEX_CLI_HOST.icon_id(), "codex-cli");
    assert_eq!(CODEX_CLI_HOST.kind(), HostKind::CliTool);
    assert_eq!(CODEX_CLI_HOST.config_format(), ConfigFormat::Toml);
    assert!(
        CODEX_CLI_HOST.download_url().starts_with("https://"),
        "the download URL is offered: {}",
        CODEX_CLI_HOST.download_url()
    );
    assert!(
        CODEX_CLI_HOST.description().contains("managed configuration"),
        "{}",
        CODEX_CLI_HOST.description()
    );
    assert!(
        CODEX_CLI_HOST.accepted_surfaces().is_empty(),
        "Codex accepts every provider surface"
    );
    assert!(
        !CODEX_CLI_HOST.install_action_label().is_empty(),
        "the install action is labelled for the current platform"
    );
}

#[test]
fn the_codex_schema_requires_the_provider_and_auth_keys() {
    let schema = CODEX_CLI_HOST.config_schema();
    assert!(
        schema
            .required_keys
            .contains(&"model_providers.systemprompt.auth.command"),
        "{:?}",
        schema.required_keys
    );
    assert!(
        schema.required_keys.contains(&"model_provider"),
        "{:?}",
        schema.required_keys
    );
    for key in schema.required_keys {
        assert!(
            schema.display_keys.contains(key),
            "{key} is required but never displayed"
        );
    }
}
