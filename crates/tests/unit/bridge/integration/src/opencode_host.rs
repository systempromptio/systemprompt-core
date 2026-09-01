use std::collections::BTreeMap;
use std::path::Path;

use systemprompt_bridge::integration::host_app::{
    AppInstallState, ConfigFormat, HostApp, HostKind, ProbeEnv, ProfileGenInputs, ProfileState,
    StaleReason,
};
use systemprompt_bridge::integration::opencode::OPENCODE_HOST;
use systemprompt_models::profile::ApiSurface;
use tempfile::TempDir;

fn probe_env() -> ProbeEnv {
    ProbeEnv {
        proxy_port: systemprompt_bridge::proxy::DEFAULT_PROXY_PORT,
        loopback_secret_fingerprint: None,
    }
}

fn sandbox<R>(managed_json: Option<&str>, f: impl FnOnce(&Path) -> R) -> R {
    let root = TempDir::new().expect("sandbox");
    let managed = root.path().join("managed");
    std::fs::create_dir_all(&managed).expect("managed dir");
    if let Some(body) = managed_json {
        std::fs::write(managed.join("opencode.json"), body).expect("seed managed config");
    }
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(root.path().display().to_string())),
        (
            "XDG_CONFIG_HOME",
            Some(root.path().join("config").display().to_string()),
        ),
        (
            "XDG_DATA_HOME",
            Some(root.path().join("data").display().to_string()),
        ),
        (
            "SP_BRIDGE_OPENCODE_MANAGED_DIR",
            Some(managed.display().to_string()),
        ),
        ("PATH", Some(root.path().join("bin").display().to_string())),
    ];
    let out = temp_env::with_vars(vars, || f(root.path()));
    drop(root);
    out
}

const COMPLETE: &str = r#"{
  "$schema": "https://opencode.ai/config.json",
  "provider": {
    "systemprompt": {
      "npm": "@ai-sdk/openai-compatible",
      "options": {
        "baseURL": "http://127.0.0.1:48217/v1",
        "headers": { "x-inference-protocol": "openai" }
      },
      "models": { "gpt-4.1": { "name": "gpt-4.1" }, "claude-sonnet-5": { "name": "claude-sonnet-5" } }
    }
  },
  "model": "systemprompt/claude-sonnet-5"
}"#;

#[test]
fn an_absent_managed_config_probes_as_absent() {
    let snapshot = sandbox(None, |_| OPENCODE_HOST.probe(&probe_env()));
    assert_eq!(snapshot.host_id, "opencode");
    assert!(
        matches!(snapshot.profile_state, ProfileState::Absent),
        "no managed file means an absent profile, got {:?}",
        snapshot.profile_state
    );
    assert!(snapshot.profile_source.is_none());
    assert!(snapshot.profile_keys.is_empty());
    assert_eq!(
        snapshot.app_installed,
        AppInstallState::NotInstalled,
        "an empty PATH with no known prefix holding `opencode` is a conclusive miss"
    );
}

#[test]
fn a_complete_managed_config_probes_as_installed_with_models_listed_by_name() {
    let snapshot = sandbox(Some(COMPLETE), |_| OPENCODE_HOST.probe(&probe_env()));
    assert!(
        matches!(snapshot.profile_state, ProfileState::Installed),
        "{:?}",
        snapshot.profile_state
    );
    let keys = &snapshot.profile_keys;
    assert_eq!(
        keys.get("provider.systemprompt.npm").map(String::as_str),
        Some("@ai-sdk/openai-compatible")
    );
    assert_eq!(
        keys.get("provider.systemprompt.options.baseURL")
            .map(String::as_str),
        Some("http://127.0.0.1:48217/v1")
    );
    assert_eq!(
        keys.get("provider.systemprompt.options.headers.x-inference-protocol")
            .map(String::as_str),
        Some("openai")
    );
    // Why: model ids carry dots, so the models object is shown as its sorted
    // key list rather than addressed per model.
    assert_eq!(
        keys.get("provider.systemprompt.models").map(String::as_str),
        Some("claude-sonnet-5, gpt-4.1")
    );
    assert_eq!(
        keys.get("model").map(String::as_str),
        Some("systemprompt/claude-sonnet-5")
    );
    assert!(
        snapshot
            .profile_source
            .as_deref()
            .is_some_and(|p| p.ends_with("opencode.json")),
        "{:?}",
        snapshot.profile_source
    );
}

#[test]
fn a_partial_managed_config_lists_the_missing_required_keys() {
    let snapshot = sandbox(
        Some(r#"{ "provider": { "systemprompt": { "npm": "@ai-sdk/openai-compatible" } } }"#),
        |_| OPENCODE_HOST.probe(&probe_env()),
    );
    match snapshot.profile_state {
        ProfileState::Partial { missing_required } => assert_eq!(
            missing_required,
            vec!["provider.systemprompt.options.baseURL".to_owned()]
        ),
        other => panic!("expected Partial, got {other:?}"),
    }
}

#[test]
fn a_stale_loopback_port_is_reported_as_stale() {
    let body = COMPLETE.replace("48217", "1");
    let snapshot = sandbox(Some(&body), |_| OPENCODE_HOST.probe(&probe_env()));
    assert!(
        matches!(
            snapshot.profile_state,
            ProfileState::Stale {
                reason: StaleReason::ProxyPort
            }
        ),
        "a baseURL on another loopback port is stale, got {:?}",
        snapshot.profile_state
    );
}

#[test]
fn a_malformed_managed_config_falls_back_to_an_empty_read() {
    let snapshot = sandbox(Some("{ this is [not json"), |_| {
        OPENCODE_HOST.probe(&probe_env())
    });
    assert!(matches!(snapshot.profile_state, ProfileState::Absent));
    assert!(snapshot.profile_keys.is_empty());
}

#[test]
fn a_jsonc_only_managed_dir_is_reported_as_the_source_but_never_read() {
    let snapshot = sandbox(None, |root| {
        let managed = root.join("managed");
        std::fs::write(
            managed.join("opencode.jsonc"),
            "// comment\n{ \"provider\": { \"systemprompt\": {} } }",
        )
        .expect("seed jsonc");
        OPENCODE_HOST.probe(&probe_env())
    });
    assert!(matches!(snapshot.profile_state, ProfileState::Absent));
    assert!(
        snapshot
            .profile_source
            .as_deref()
            .is_some_and(|p| p.ends_with("opencode.jsonc")),
        "{:?}",
        snapshot.profile_source
    );
}

#[test]
fn a_user_scope_provider_block_is_not_governance() {
    let snapshot = sandbox(None, |root| {
        let user = root.join("config").join("opencode");
        std::fs::create_dir_all(&user).expect("user dir");
        std::fs::write(user.join("opencode.json"), COMPLETE).expect("seed user config");
        OPENCODE_HOST.probe(&probe_env())
    });
    assert!(
        matches!(snapshot.profile_state, ProfileState::Absent),
        "a provider block the user can edit must not read as installed: {:?}",
        snapshot.profile_state
    );
}

#[test]
fn the_binary_is_found_in_a_known_install_prefix_outside_path() {
    let snapshot = sandbox(None, |root| {
        let bin = root.join(".opencode").join("bin");
        std::fs::create_dir_all(&bin).expect("prefix");
        std::fs::write(bin.join("opencode"), "#!/bin/sh\n").expect("stub binary");
        OPENCODE_HOST.probe(&probe_env())
    });
    assert_eq!(snapshot.app_installed, AppInstallState::Installed);
}

#[test]
fn the_opencode_host_describes_itself_as_a_json_cli_tool_that_cannot_be_opened() {
    assert_eq!(OPENCODE_HOST.id(), "opencode");
    assert_eq!(OPENCODE_HOST.display_name(), "OpenCode");
    assert_eq!(OPENCODE_HOST.icon_id(), "opencode");
    assert_eq!(OPENCODE_HOST.kind(), HostKind::CliTool);
    assert_eq!(OPENCODE_HOST.config_format(), ConfigFormat::Json);
    assert!(!OPENCODE_HOST.can_open());
    assert!(OPENCODE_HOST.download_url().starts_with("https://"));
    assert!(
        OPENCODE_HOST.description().contains("OpenCode"),
        "{}",
        OPENCODE_HOST.description()
    );
    assert_eq!(OPENCODE_HOST.accepted_surfaces(), &[ApiSurface::OpenAi]);
    assert!(
        OPENCODE_HOST
            .install_action_label()
            .contains("opencode.json")
    );
}

#[test]
fn the_opencode_schema_requires_the_wire_and_the_loopback_endpoint() {
    let schema = OPENCODE_HOST.config_schema();
    assert_eq!(
        schema.required_keys,
        &[
            "provider.systemprompt.npm",
            "provider.systemprompt.options.baseURL"
        ]
    );
    assert!(schema.display_keys.contains(&"model"));
}

#[test]
fn generating_a_profile_carries_the_provider_block_and_the_key_marker() {
    let mut headers = BTreeMap::new();
    headers.insert("x-inference-protocol".to_owned(), "openai".to_owned());
    let generated = sandbox(None, |_| {
        OPENCODE_HOST
            .generate_profile(&ProfileGenInputs {
                gateway_base_url: "http://127.0.0.1:48217/".to_owned(),
                api_key: "loopback-secret-value".to_owned(),
                models: vec!["claude-sonnet-5".to_owned(), "gpt-4.1".to_owned()],
                organization_uuid: None,
                headers,
            })
            .expect("profile generated")
    });
    let body = std::fs::read_to_string(&generated.path).expect("generated readable");
    let doc: serde_json::Value = serde_json::from_str(&body).expect("generated is JSON");
    assert_eq!(
        doc["provider"]["systemprompt"]["options"]["baseURL"],
        "http://127.0.0.1:48217/v1"
    );
    assert_eq!(
        doc["provider"]["systemprompt"]["options"]["headers"]["x-inference-protocol"],
        "openai"
    );
    assert_eq!(
        doc["provider"]["systemprompt"]["models"]["gpt-4.1"]["name"],
        "gpt-4.1"
    );
    assert_eq!(doc["model"], "systemprompt/claude-sonnet-5");
    assert_eq!(doc["_systemprompt_api_key"], "loopback-secret-value");
    assert_eq!(generated.bytes, body.len());
    assert_ne!(generated.payload_uuid, generated.profile_uuid);
    _ = std::fs::remove_file(&generated.path);
}
