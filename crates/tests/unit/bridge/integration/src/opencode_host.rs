use std::collections::BTreeMap;
use std::path::Path;

use systemprompt_bridge::ids::HostToken;
use systemprompt_bridge::integration::host_app::{
    AppInstallState, ConfigFormat, HostApp, HostAppKind, ProfileGenInputs, ProfileState,
    StaleReason,
};
use systemprompt_bridge::integration::opencode::{OPENCODE_HOST, install_state};
use systemprompt_models::bridge::host::HostKind;
use tempfile::TempDir;

use crate::support::{host_token, probe_env};

fn with_key(body: &str) -> String {
    let mut doc: serde_json::Value = serde_json::from_str(body).expect("seed is JSON");
    doc["provider"]["systemprompt"]["options"]["apiKey"] =
        serde_json::Value::String(host_token(HostKind::OpenCode));
    doc.to_string()
}

fn write_bridge_config(config_home: &Path, managed_dir: &Path) {
    let dir = config_home.join("systemprompt");
    std::fs::create_dir_all(&dir).expect("bridge config dir");
    std::fs::write(
        dir.join("systemprompt-bridge.toml"),
        format!("[opencode]\nmanaged_dir = '{}'\n", managed_dir.display()),
    )
    .expect("bridge config");
}

fn sandbox<R>(user_json: Option<&str>, f: impl FnOnce(&Path) -> R) -> R {
    let root = TempDir::new().expect("sandbox");
    let managed = root.path().join("managed");
    std::fs::create_dir_all(&managed).expect("managed dir");
    let user = root.path().join("config").join("opencode");
    std::fs::create_dir_all(&user).expect("user dir");
    if let Some(body) = user_json {
        std::fs::write(user.join("opencode.json"), body).expect("seed user config");
    }
    write_bridge_config(&root.path().join("config"), &managed);
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
        ("SP_BRIDGE_CONFIG", None),
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
  "model": "systemprompt/claude-sonnet-5",
  "enabled_providers": ["systemprompt"]
}"#;

#[test]
fn an_absent_config_probes_as_absent() {
    let snapshot = sandbox(None, |_| OPENCODE_HOST.probe(&probe_env()));
    assert_eq!(snapshot.host_id, HostKind::OpenCode);
    assert!(
        matches!(snapshot.profile_state, ProfileState::Absent),
        "no user config means an absent profile, got {:?}",
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
fn a_config_without_its_api_key_is_partial_with_models_listed_by_name() {
    let snapshot = sandbox(Some(COMPLETE), |_| OPENCODE_HOST.probe(&probe_env()));
    assert!(
        matches!(
            &snapshot.profile_state,
            ProfileState::Partial { missing_required } if missing_required.contains(&"provider.systemprompt.options.apiKey".to_owned())
        ),
        "no host token in the provider block means OpenCode cannot authenticate: {:?}",
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
    assert_eq!(
        keys.get("enabled_providers").map(String::as_str),
        Some(r#"["systemprompt"]"#)
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
fn a_config_without_the_endpoint_is_partial() {
    let snapshot = sandbox(
        Some(r#"{ "provider": { "systemprompt": { "npm": "@ai-sdk/openai-compatible" } } }"#),
        |_| OPENCODE_HOST.probe(&probe_env()),
    );
    match snapshot.profile_state {
        ProfileState::Partial { missing_required } => assert!(
            missing_required.contains(&"provider.systemprompt.options.baseURL".to_owned())
                && missing_required.contains(&"enabled_providers".to_owned()),
            "{missing_required:?}"
        ),
        other => panic!("expected Partial, got {other:?}"),
    }
}

#[test]
fn a_stale_loopback_port_is_reported_as_stale() {
    let body = with_key(&COMPLETE.replace("48217", "1"));
    let snapshot = sandbox(Some(&body), |_| OPENCODE_HOST.probe(&probe_env()));
    assert!(
        matches!(
            &snapshot.profile_state,
            ProfileState::Stale {
                reason: StaleReason::ProxyPort,
                keys,
            } if keys.contains(&"provider.systemprompt.options.baseURL".to_owned())
        ),
        "a baseURL on another loopback port is stale, got {:?}",
        snapshot.profile_state
    );
}

#[test]
fn a_malformed_config_is_unverifiable_not_absent() {
    let snapshot = sandbox(Some("{ this is [not json"), |_| {
        OPENCODE_HOST.probe(&probe_env())
    });
    assert!(
        matches!(&snapshot.profile_state, ProfileState::Unverifiable { reason } if reason.contains("opencode.json")),
        "a file the probe cannot read is not an absent profile: {:?}",
        snapshot.profile_state
    );
    assert!(snapshot.profile_keys.is_empty());
    assert!(snapshot.probe_error.is_some());
}

#[test]
fn a_jsonc_only_user_dir_is_reported_as_the_source_but_never_read() {
    let snapshot = sandbox(None, |root| {
        std::fs::write(
            root.join("config").join("opencode").join("opencode.jsonc"),
            "// comment\n{ \"provider\": { \"systemprompt\": {} } }",
        )
        .expect("seed jsonc");
        OPENCODE_HOST.probe(&probe_env())
    });
    assert!(
        matches!(snapshot.profile_state, ProfileState::Unverifiable { .. }),
        "a jsonc tier the bridge cannot parse is reported, not read as absent: {:?}",
        snapshot.profile_state
    );
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
fn a_config_without_the_provider_allowlist_is_not_installed() {
    let unrestricted = with_key(&COMPLETE.replace(
        r#",
  "enabled_providers": ["systemprompt"]"#,
        "",
    ));
    let snapshot = sandbox(Some(&unrestricted), |_| OPENCODE_HOST.probe(&probe_env()));
    match snapshot.profile_state {
        ProfileState::Stale { keys, .. } => {
            assert!(keys.contains(&"enabled_providers".to_owned()), "{keys:?}");
        },
        other => {
            panic!("a file that lets OpenCode's own providers through is not governed: {other:?}")
        },
    }
}

#[test]
fn either_install_of_the_host_counts_and_only_a_double_miss_is_a_miss() {
    use AppInstallState::{Installed, NotInstalled, Unknown};
    assert_eq!(install_state(Installed, NotInstalled), Installed);
    assert_eq!(install_state(NotInstalled, Installed), Installed);
    assert_eq!(install_state(Unknown, Installed), Installed);
    assert_eq!(install_state(NotInstalled, NotInstalled), NotInstalled);
    assert_eq!(install_state(Unknown, NotInstalled), Unknown);
    assert_eq!(install_state(NotInstalled, Unknown), Unknown);
}

#[test]
fn the_opencode_host_describes_itself_as_a_json_cli_tool_openable_only_with_the_desktop_app() {
    assert_eq!(OPENCODE_HOST.id(), HostKind::OpenCode);
    assert_eq!(OPENCODE_HOST.display_name(), "OpenCode");
    assert_eq!(OPENCODE_HOST.icon_id(), "opencode");
    assert_eq!(OPENCODE_HOST.kind(), HostAppKind::CliTool);
    assert_eq!(OPENCODE_HOST.config_format(), ConfigFormat::Json);
    assert!(
        !OPENCODE_HOST.can_open(),
        "with no desktop bundle on this machine there is nothing to open"
    );
    assert!(OPENCODE_HOST.download_url().starts_with("https://"));
    assert!(
        OPENCODE_HOST.description().contains("OpenCode"),
        "{}",
        OPENCODE_HOST.description()
    );
    assert_eq!(
        systemprompt_bridge::gateway::model_view::host_client(OPENCODE_HOST.id()),
        systemprompt_models::providers::ClientFamily::OpenAi,
        "OpenCode lists the models the catalog advertises to openai clients"
    );
    assert!(
        OPENCODE_HOST
            .install_action_label()
            .contains("opencode.json")
    );
}

#[test]
fn generating_a_profile_carries_the_provider_block_and_its_api_key() {
    let mut headers = BTreeMap::new();
    headers.insert("x-inference-protocol".to_owned(), "openai".to_owned());
    let generated = sandbox(None, |_| {
        OPENCODE_HOST
            .generate_profile(&ProfileGenInputs {
                model_limits: Default::default(),
                gateway_base_url: "http://127.0.0.1:48217/".to_owned(),
                host_token: HostToken::new("loopback-secret-value"),
                models: vec!["claude-sonnet-5".to_owned(), "gpt-4.1".to_owned()],
                default_model: None,
                organization_uuid: None,
                headers,
                mcp_servers: Some(Vec::new()),
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
    assert_eq!(
        doc["enabled_providers"],
        serde_json::json!(["systemprompt"])
    );
    assert_eq!(
        doc["provider"]["systemprompt"]["options"]["apiKey"],
        "loopback-secret-value"
    );
    assert_eq!(generated.bytes, body.len());
    assert_ne!(generated.payload_uuid, generated.profile_uuid);
    _ = std::fs::remove_file(&generated.path);
}
