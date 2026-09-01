use systemprompt_bridge::integration::hermes::HERMES_HOST;
use systemprompt_bridge::integration::host_app::{ConfigFormat, HostApp, HostKind, ProfileState};
use tempfile::TempDir;

fn hermes_sandbox<R>(config_yaml: Option<&str>, f: impl FnOnce() -> R) -> R {
    let home = TempDir::new().expect("hermes home");
    if let Some(body) = config_yaml {
        std::fs::write(home.path().join("config.yaml"), body).expect("seed hermes config");
    }
    let vars: Vec<(&str, Option<String>)> =
        vec![("HERMES_HOME", Some(home.path().display().to_string()))];
    let out = temp_env::with_vars(vars, f);
    drop(home);
    out
}

fn loopback_v1() -> String {
    format!(
        "http://127.0.0.1:{}/v1",
        systemprompt_bridge::proxy::resolved_port()
    )
}

#[test]
fn an_absent_hermes_config_probes_as_absent() {
    let snapshot = hermes_sandbox(None, || HERMES_HOST.probe());
    assert_eq!(snapshot.host_id, "hermes");
    assert!(
        matches!(snapshot.profile_state, ProfileState::Absent),
        "no config.yaml means an absent profile, got {:?}",
        snapshot.profile_state
    );
    assert!(
        snapshot.profile_source.is_none(),
        "no file was read, got {:?}",
        snapshot.profile_source
    );
    assert!(
        snapshot.profile_keys.is_empty(),
        "no keys without a file, got {:?}",
        snapshot.profile_keys
    );
}

#[test]
fn a_complete_hermes_config_probes_as_installed_with_the_model_keys() {
    let base_url = loopback_v1();
    let body = format!(
        "model:\n  provider: systemprompt-gateway\n  default: gpt-5\nproviders:\n  \
         systemprompt-gateway:\n    base_url: {base_url}\n    api_mode: chat_completions\n    \
         key_env: OPENAI_API_KEY\nother: kept\n"
    );
    let snapshot = hermes_sandbox(Some(&body), || HERMES_HOST.probe());
    assert!(
        matches!(snapshot.profile_state, ProfileState::Installed),
        "every required key is present, got {:?}",
        snapshot.profile_state
    );
    assert_eq!(
        snapshot
            .profile_keys
            .get("providers.systemprompt-gateway.base_url")
            .map(String::as_str),
        Some(base_url.as_str()),
        "{:?}",
        snapshot.profile_keys
    );
    assert_eq!(
        snapshot
            .profile_keys
            .get("providers.systemprompt-gateway.api_mode")
            .map(String::as_str),
        Some("chat_completions"),
        "{:?}",
        snapshot.profile_keys
    );
    assert_eq!(
        snapshot
            .profile_keys
            .get("model.default")
            .map(String::as_str),
        Some("gpt-5"),
        "{:?}",
        snapshot.profile_keys
    );
    assert!(
        !snapshot.profile_keys.contains_key("other"),
        "only the bridge-owned keys are surfaced, got {:?}",
        snapshot.profile_keys
    );
    assert!(
        snapshot
            .profile_source
            .as_deref()
            .is_some_and(|s| s.ends_with("config.yaml")),
        "the probe reports which file it read, got {:?}",
        snapshot.profile_source
    );
}

#[test]
fn a_partial_hermes_config_lists_the_missing_required_keys() {
    let body = format!(
        "model:\n  provider: systemprompt-gateway\nproviders:\n  systemprompt-gateway:\n    \
         base_url: {}\n",
        loopback_v1()
    );
    let snapshot = hermes_sandbox(Some(&body), || HERMES_HOST.probe());
    match snapshot.profile_state {
        ProfileState::Partial { missing_required } => {
            assert_eq!(
                missing_required,
                vec![
                    "providers.systemprompt-gateway.api_mode".to_owned(),
                    "providers.systemprompt-gateway.key_env".to_owned()
                ],
                "the wire format and the key source are missing: {missing_required:?}"
            );
        },
        other => panic!("expected Partial, got {other:?}"),
    }
}

#[test]
fn a_malformed_hermes_config_falls_back_to_an_empty_read() {
    let snapshot = hermes_sandbox(Some("model: [not: yaml\n  : :"), || HERMES_HOST.probe());
    assert!(
        matches!(snapshot.profile_state, ProfileState::Absent),
        "a YAML parse failure degrades to Absent, got {:?}",
        snapshot.profile_state
    );
    assert!(
        snapshot.profile_source.is_none(),
        "an unparseable file is not reported as a source, got {:?}",
        snapshot.profile_source
    );
    assert!(
        snapshot.profile_keys.is_empty(),
        "no keys survive a parse failure, got {:?}",
        snapshot.profile_keys
    );
}

#[test]
fn the_hermes_host_describes_itself_as_a_yaml_desktop_app() {
    assert_eq!(HERMES_HOST.id(), "hermes");
    assert_eq!(HERMES_HOST.display_name(), "Hermes");
    assert_eq!(HERMES_HOST.icon_id(), "hermes");
    assert_eq!(HERMES_HOST.kind(), HostKind::DesktopApp);
    assert_eq!(HERMES_HOST.config_format(), ConfigFormat::Yaml);
    assert!(
        HERMES_HOST.download_url().starts_with("https://"),
        "the download URL is offered: {}",
        HERMES_HOST.download_url()
    );
    assert!(
        HERMES_HOST.description().contains("managed configuration"),
        "{}",
        HERMES_HOST.description()
    );
    assert_eq!(
        HERMES_HOST.accepted_surfaces(),
        &[systemprompt_models::profile::ApiSurface::OpenAi],
        "Hermes speaks the OpenAI API surface"
    );
    assert!(HERMES_HOST.can_open(), "a desktop app can be opened");
    assert!(
        !HERMES_HOST.install_action_label().is_empty(),
        "the install action is labelled"
    );
}

#[test]
fn the_hermes_schema_requires_the_provider_selection_endpoint_wire_and_key_source() {
    let schema = HERMES_HOST.config_schema();
    assert_eq!(
        schema.required_keys,
        &[
            "model.provider",
            "providers.systemprompt-gateway.base_url",
            "providers.systemprompt-gateway.api_mode",
            "providers.systemprompt-gateway.key_env"
        ],
        "{:?}",
        schema.required_keys
    );
    for key in schema.required_keys {
        assert!(
            schema.display_keys.contains(key),
            "{key} is required but never displayed"
        );
    }
    assert!(
        schema.display_keys.contains(&"model.default"),
        "the optional model name is displayed: {:?}",
        schema.display_keys
    );
}
