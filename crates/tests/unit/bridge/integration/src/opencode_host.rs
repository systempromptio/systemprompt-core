use std::collections::BTreeMap;
use std::path::Path;

use systemprompt_bridge::ids::HostToken;
use systemprompt_bridge::integration::host_app::{
    AppInstallState, ConfigFormat, HostApp, HostAppKind, ProfileGenInputs, ProfileState,
    StaleReason,
};
use systemprompt_bridge::integration::opencode::OPENCODE_CLI_HOST;
use systemprompt_models::bridge::host::HostKind;
use tempfile::TempDir;

use crate::support::{host_token, probe_env};

fn write_token(root: &Path) {
    std::fs::write(
        root.join("config")
            .join("opencode")
            .join("systemprompt.token"),
        host_token(HostKind::OpenCode),
    )
    .expect("seed token");
}

fn api_key(root: &Path) -> String {
    format!(
        "{{file:{}}}",
        root.join("config")
            .join("opencode")
            .join("systemprompt.token")
            .display()
    )
}

fn complete(root: &Path) -> String {
    COMPLETE.replace("__API_KEY__", &api_key(root))
}

fn seed(root: &Path, body: &str) {
    std::fs::write(
        root.join("config").join("opencode").join("opencode.json"),
        body,
    )
    .expect("seed user config");
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
  "providers": {
    "systemprompt": {
      "name": "systemprompt",
      "package": "aisdk:@ai-sdk/openai-compatible",
      "settings": {
        "baseURL": "http://127.0.0.1:48217/v1",
        "apiKey": "__API_KEY__",
        "headers": { "x-inference-protocol": "openai" }
      },
      "models": { "gpt-4.1": { "modelID": "gpt-4.1" }, "claude-sonnet-5": { "modelID": "claude-sonnet-5" } }
    }
  },
  "model": "systemprompt/claude-sonnet-5",
  "policies": [
    { "effect": "deny", "action": "provider.use", "resource": "*" },
    { "effect": "allow", "action": "provider.use", "resource": "systemprompt" }
  ]
}"#;

#[test]
fn an_absent_config_probes_as_absent() {
    let snapshot = sandbox(None, |_| OPENCODE_CLI_HOST.probe(&probe_env()));
    assert_eq!(snapshot.host_id, HostKind::OpenCodeCli);
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
fn a_config_without_its_token_file_is_partial_with_models_listed_by_name() {
    let snapshot = sandbox(None, |root| {
        seed(root, &complete(root));
        OPENCODE_CLI_HOST.probe(&probe_env())
    });
    assert!(
        matches!(
            &snapshot.profile_state,
            ProfileState::Partial { missing_required } if missing_required.contains(&"systemprompt.token".to_owned())
        ),
        "no token file means OpenCode cannot authenticate: {:?}",
        snapshot.profile_state
    );
    let keys = &snapshot.profile_keys;
    assert_eq!(
        keys.get("providers.systemprompt.package")
            .map(String::as_str),
        Some("aisdk:@ai-sdk/openai-compatible")
    );
    assert_eq!(
        keys.get("providers.systemprompt.settings.baseURL")
            .map(String::as_str),
        Some("http://127.0.0.1:48217/v1")
    );
    assert_eq!(
        keys.get("providers.systemprompt.settings.headers.x-inference-protocol")
            .map(String::as_str),
        Some("openai")
    );
    assert_eq!(
        keys.get("providers.systemprompt.models")
            .map(String::as_str),
        Some("claude-sonnet-5, gpt-4.1")
    );
    assert_eq!(
        keys.get("model").map(String::as_str),
        Some("systemprompt/claude-sonnet-5")
    );
}

#[test]
fn a_config_without_the_endpoint_is_partial() {
    let snapshot = sandbox(
        Some(
            r#"{ "providers": { "systemprompt": { "package": "aisdk:@ai-sdk/openai-compatible" } } }"#,
        ),
        |_| OPENCODE_CLI_HOST.probe(&probe_env()),
    );
    match snapshot.profile_state {
        ProfileState::Partial { missing_required } => assert!(
            missing_required.contains(&"providers.systemprompt.settings.baseURL".to_owned())
                && missing_required.contains(&"policies".to_owned()),
            "{missing_required:?}"
        ),
        other => panic!("expected Partial, got {other:?}"),
    }
}

#[test]
fn a_stale_loopback_port_is_reported_as_stale() {
    let snapshot = sandbox(None, |root| {
        seed(root, &complete(root).replace("48217", "1"));
        write_token(root);
        OPENCODE_CLI_HOST.probe(&probe_env())
    });
    assert!(
        matches!(
            &snapshot.profile_state,
            ProfileState::Stale {
                reason: StaleReason::ProxyPort,
                keys,
            } if keys.contains(&"providers.systemprompt.settings.baseURL".to_owned())
        ),
        "a baseURL on another loopback port is stale, got {:?}",
        snapshot.profile_state
    );
}

#[test]
fn a_malformed_config_is_unverifiable_not_absent() {
    let snapshot = sandbox(Some("{ this is [not json"), |_| {
        OPENCODE_CLI_HOST.probe(&probe_env())
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
        OPENCODE_CLI_HOST.probe(&probe_env())
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
        OPENCODE_CLI_HOST.probe(&probe_env())
    });
    assert_eq!(snapshot.app_installed, AppInstallState::Installed);
}

#[test]
fn a_config_without_the_provider_policies_is_not_installed() {
    let snapshot = sandbox(None, |root| {
        let mut doc: serde_json::Value =
            serde_json::from_str(&complete(root)).expect("fixture is JSON");
        doc.as_object_mut().expect("object").remove("policies");
        seed(root, &doc.to_string());
        write_token(root);
        OPENCODE_CLI_HOST.probe(&probe_env())
    });
    match snapshot.profile_state {
        ProfileState::Stale { keys, .. }
        | ProfileState::Partial {
            missing_required: keys,
        } => {
            assert!(keys.contains(&"policies".to_owned()), "{keys:?}");
        },
        other => {
            panic!("a file that lets OpenCode's own providers through is not governed: {other:?}")
        },
    }
}

#[test]
fn the_opencode_cli_host_describes_itself_as_a_json_cli_tool_that_cannot_be_opened() {
    assert_eq!(OPENCODE_CLI_HOST.id(), HostKind::OpenCodeCli);
    assert_eq!(OPENCODE_CLI_HOST.display_name(), "OpenCode CLI");
    assert_eq!(OPENCODE_CLI_HOST.icon_id(), HostKind::OpenCode.as_str());
    assert_eq!(OPENCODE_CLI_HOST.kind(), HostAppKind::CliTool);
    assert_eq!(OPENCODE_CLI_HOST.config_format(), ConfigFormat::Json);
    assert!(!OPENCODE_CLI_HOST.can_open(), "a CLI has no window to open");
    assert!(OPENCODE_CLI_HOST.download_url().starts_with("https://"));
    assert!(
        OPENCODE_CLI_HOST.description().contains("OpenCode"),
        "{}",
        OPENCODE_CLI_HOST.description()
    );
    assert_eq!(
        systemprompt_bridge::gateway::model_view::host_client(OPENCODE_CLI_HOST.id()),
        systemprompt_models::providers::ClientFamily::OpenAi,
        "OpenCode lists the models the catalog advertises to openai clients"
    );
    assert!(
        OPENCODE_CLI_HOST
            .install_action_label()
            .contains("opencode.json")
    );
}

#[test]
fn generating_a_profile_carries_the_v2_provider_block_and_its_token() {
    let mut headers = BTreeMap::new();
    headers.insert("x-inference-protocol".to_owned(), "openai".to_owned());
    let generated = sandbox(None, |_| {
        OPENCODE_CLI_HOST
            .generate_profile(&ProfileGenInputs {
                model_limits: Default::default(),
                model_names: Default::default(),
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
    let generated_doc: serde_json::Value = serde_json::from_str(&body).expect("generated is JSON");
    assert_eq!(generated_doc["token"], "loopback-secret-value");
    let doc = &generated_doc["config"];
    let ours = &doc["providers"]["systemprompt"];
    assert_eq!(ours["settings"]["baseURL"], "http://127.0.0.1:48217/v1");
    assert_eq!(
        ours["settings"]["headers"]["x-inference-protocol"],
        "openai"
    );
    assert_eq!(ours["models"]["gpt-4.1"]["modelID"], "gpt-4.1");
    assert!(
        ours["settings"]["apiKey"]
            .as_str()
            .is_some_and(|key| key.starts_with("{file:") && !key.contains("loopback-secret")),
        "{ours}"
    );
    assert_eq!(doc["model"], "systemprompt/claude-sonnet-5");
    assert_eq!(doc["policies"][1]["resource"], "systemprompt");
    assert_eq!(generated.bytes, body.len());
    assert_ne!(generated.payload_uuid, generated.profile_uuid);
    _ = std::fs::remove_file(&generated.path);
}
