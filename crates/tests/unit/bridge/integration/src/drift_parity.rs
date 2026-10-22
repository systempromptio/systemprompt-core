use std::path::Path;

#[cfg(not(target_os = "macos"))]
use systemprompt_bridge::integration::codex::CODEX_CLI_HOST;
use systemprompt_bridge::integration::hermes::HERMES_HOST;
use systemprompt_bridge::integration::host_app::{
    HostApp, ProfileGenInputs, ProfileState, StaleReason,
};
use systemprompt_bridge::integration::opencode::OPENCODE_CLI_HOST as OPENCODE_HOST;
use tempfile::TempDir;

use crate::support::{inputs_for, probe_env};

fn install(host: &dyn HostApp) {
    let generated = host
        .generate_profile(&inputs_for(host))
        .expect("profile generated");
    host.install_profile(&generated.path)
        .expect("profile installed");
    _ = std::fs::remove_file(&generated.path);
}

fn assert_installed_then_stale_on(
    host: &dyn HostApp,
    file: &Path,
    key: &str,
    mutate: impl Fn(&str) -> String,
) {
    install(host);
    let state = host.probe(&probe_env()).profile_state;
    assert!(
        matches!(state, ProfileState::Installed),
        "{}: what the writer wrote is exactly what the probe expects, got {state:?}",
        host.display_name()
    );
    let body = std::fs::read_to_string(file).expect("installed file readable");
    std::fs::write(file, mutate(&body)).expect("mutate installed file");
    match host.probe(&probe_env()).profile_state {
        ProfileState::Stale { reason, keys } => {
            assert_eq!(reason, StaleReason::Settings, "{}", host.display_name());
            assert_eq!(keys, vec![key.to_owned()], "{}", host.display_name());
        },
        other => panic!("{}: expected Stale, got {other:?}", host.display_name()),
    }
}

fn opencode_sandbox<R>(f: impl FnOnce(&Path) -> R) -> R {
    let root = TempDir::new().expect("sandbox");
    let managed = root.path().join("managed");
    std::fs::create_dir_all(&managed).expect("managed dir");
    let bridge = root.path().join("config").join("systemprompt");
    std::fs::create_dir_all(&bridge).expect("bridge config dir");
    std::fs::write(
        bridge.join("systemprompt-bridge.toml"),
        format!("[opencode]\nmanaged_dir = '{}'\n", managed.display()),
    )
    .expect("bridge config");
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
    ];
    let user = root
        .path()
        .join("config")
        .join("opencode")
        .join("opencode.json");
    temp_env::with_vars(vars, || f(&user))
}

fn hermes_sandbox<R>(f: impl FnOnce(&Path) -> R) -> R {
    let home = TempDir::new().expect("hermes home");
    let config = home.path().join("config.yaml");
    temp_env::with_var(
        "HERMES_HOME",
        Some(home.path().display().to_string()),
        || f(&config),
    )
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn codex_sandbox<R>(f: impl FnOnce(&Path) -> R) -> R {
    let home = TempDir::new().expect("codex home");
    let managed = home.path().join("etc").join("config.toml");
    std::fs::create_dir_all(managed.parent().expect("parent")).expect("etc dir");
    let vars: Vec<(&str, Option<String>)> = vec![
        ("CODEX_HOME", Some(home.path().display().to_string())),
        ("CODEX_SYSTEM_CONFIG", Some(managed.display().to_string())),
    ];
    temp_env::with_vars(vars, || f(&managed))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn codex_reads_back_what_it_wrote_and_names_a_changed_policy_value() {
    codex_sandbox(|managed| {
        assert_installed_then_stale_on(&CODEX_CLI_HOST, managed, "approval_policy", |body| {
            body.replace(
                "approval_policy = \"never\"",
                "approval_policy = \"on-request\"",
            )
        });
    });
}

#[test]
fn opencode_reads_back_what_it_wrote_and_names_a_changed_allowlist() {
    opencode_sandbox(|user| {
        assert_installed_then_stale_on(&OPENCODE_HOST, user, "enabled_providers", |body| {
            let mut doc: serde_json::Value = serde_json::from_str(body).expect("json");
            doc["enabled_providers"] = serde_json::json!(["systemprompt", "opencode"]);
            doc.to_string()
        });
    });
}

#[test]
fn hermes_reads_back_what_it_wrote_and_names_a_changed_wire() {
    hermes_sandbox(|config| {
        assert_installed_then_stale_on(
            &HERMES_HOST,
            config,
            "providers.systemprompt-gateway.api_mode",
            |body| body.replace("chat_completions", "responses"),
        );
    });
}

fn rich_inputs(host: &dyn HostApp) -> ProfileGenInputs {
    let mut inputs = inputs_for(host);
    inputs.models = vec!["gpt-5".to_owned(), "claude-sonnet-5".to_owned()];
    inputs.default_model = Some("gpt-5".to_owned());
    inputs.organization_uuid = Some("00000000-0000-4000-8000-000000000009".to_owned());
    inputs
        .headers
        .insert("x-inference-protocol".to_owned(), "openai".to_owned());
    inputs
}

fn json_leaves(prefix: &str, value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) if !map.is_empty() => {
            for (k, v) in map {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                json_leaves(&path, v, out);
            }
        },
        _ => out.push(prefix.to_owned()),
    }
}

fn assert_every_leaf_is_compared(host: &dyn HostApp, leaves: &[String], marker: &str) {
    let desired = host.desired(&rich_inputs(host)).expect("desired renders");
    let compared: Vec<&str> = desired.keys().collect();
    for leaf in leaves.iter().filter(|l| l.as_str() != marker) {
        assert!(
            compared
                .iter()
                .any(|k| leaf == k || leaf.starts_with(&format!("{k}."))),
            "{} writes `{leaf}` but its probe never compares it; compared: {compared:?}",
            host.display_name()
        );
    }
}

fn generated_text(host: &dyn HostApp) -> String {
    let generated = host
        .generate_profile(&rich_inputs(host))
        .expect("profile generated");
    let text = std::fs::read_to_string(&generated.path).expect("generated readable");
    _ = std::fs::remove_file(&generated.path);
    text
}

#[cfg(not(target_os = "macos"))]
#[test]
fn every_key_the_codex_writer_sets_is_compared_by_its_probe() {
    let text = codex_sandbox_free(|| generated_text(&CODEX_CLI_HOST));
    let doc: toml::Value = toml::from_str(&text).expect("generated is TOML");
    let json = serde_json::to_value(&doc).expect("toml as json");
    let mut leaves = Vec::new();
    json_leaves("", &json, &mut leaves);
    assert!(leaves.len() > 10, "{leaves:?}");
    assert_every_leaf_is_compared(&CODEX_CLI_HOST, &leaves, "");
}

#[cfg(not(target_os = "macos"))]
fn codex_sandbox_free<R>(f: impl FnOnce() -> R) -> R {
    let home = TempDir::new().expect("codex home");
    temp_env::with_var("CODEX_HOME", Some(home.path().display().to_string()), f)
}

#[test]
fn every_key_the_opencode_writer_sets_is_compared_by_its_probe() {
    let text = opencode_sandbox(|_| generated_text(&OPENCODE_HOST));
    let doc: serde_json::Value = serde_json::from_str(&text).expect("generated is JSON");
    let mut leaves = Vec::new();
    json_leaves("", &doc, &mut leaves);
    assert_every_leaf_is_compared(&OPENCODE_HOST, &leaves, "_systemprompt_api_key");
}

#[test]
fn every_key_the_hermes_writer_sets_is_compared_by_its_probe() {
    let text = hermes_sandbox(|_| generated_text(&HERMES_HOST));
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("generated is YAML");
    let json = serde_json::to_value(&doc).expect("yaml as json");
    let mut leaves = Vec::new();
    json_leaves("", &json, &mut leaves);
    assert_every_leaf_is_compared(&HERMES_HOST, &leaves, "_systemprompt_openai_api_key");
}
