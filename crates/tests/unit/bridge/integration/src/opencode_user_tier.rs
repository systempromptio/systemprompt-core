//! With the provider in the user tier the probe reads the user config and the
//! token file; with an admin-managed (MDM) provider only the token file.

use std::path::Path;

use systemprompt_bridge::integration::host_app::{HostApp, ProfileRemoval, ProfileState};
use systemprompt_bridge::integration::opencode::OPENCODE_CLI_HOST as OPENCODE_HOST;
use tempfile::TempDir;

use crate::support::{inputs_for, probe_env};

const MDM_BLOCK: &str = r#"{
  "providers": {
    "systemprompt": {
      "package": "aisdk:@ai-sdk/openai-compatible",
      "settings": { "baseURL": "https://gateway.example/v1", "apiKey": "{file:~/.config/opencode/systemprompt.token}" }
    }
  }
}"#;

fn write_bridge_config(config_home: &Path, managed_dir: &Path) {
    let dir = config_home
        .join("systemprompt")
        .join("gateways")
        .join("default");
    std::fs::create_dir_all(&dir).expect("bridge config dir");
    std::fs::write(
        dir.join("systemprompt-bridge.toml"),
        format!("[opencode]\nmanaged_dir = '{}'\n", managed_dir.display()),
    )
    .expect("bridge config");
}

fn sandbox<R>(managed: Option<&str>, f: impl FnOnce(&Path) -> R) -> R {
    let root = TempDir::new().expect("sandbox");
    let managed_dir = root.path().join("managed");
    std::fs::create_dir_all(&managed_dir).expect("managed dir");
    if let Some(body) = managed {
        std::fs::write(managed_dir.join("opencode.json"), body).expect("seed managed config");
    }
    let config_home = root.path().join("config");
    write_bridge_config(&config_home, &managed_dir);
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(root.path().display().to_string())),
        ("XDG_CONFIG_HOME", Some(config_home.display().to_string())),
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

fn install() {
    let generated = OPENCODE_HOST
        .generate_profile(&inputs_for(&OPENCODE_HOST))
        .expect("profile generated");
    OPENCODE_HOST
        .install_profile(
            &generated.path,
            &systemprompt_bridge::elevation_gate::ElevationGate::new(),
        )
        .expect("installed");
    _ = std::fs::remove_file(&generated.path);
}

#[test]
fn an_installed_user_tier_probes_as_installed_and_names_its_source() {
    let snapshot = sandbox(None, |_| {
        install();
        OPENCODE_HOST.probe(&probe_env())
    });
    assert!(
        matches!(snapshot.profile_state, ProfileState::Installed),
        "{:?}",
        snapshot.profile_state
    );
    assert!(
        snapshot
            .profile_source
            .as_deref()
            .is_some_and(|p| p.ends_with("opencode.json") && p.contains("config")),
        "{:?}",
        snapshot.profile_source
    );
    assert!(
        !snapshot.update_needs_approval,
        "the user tier never needs elevation"
    );
}

#[test]
fn the_probe_never_shows_the_api_key() {
    let snapshot = sandbox(None, |_| {
        install();
        OPENCODE_HOST.probe(&probe_env())
    });
    let shown = snapshot
        .profile_keys
        .get("systemprompt.token")
        .expect("the token's presence is reported");
    let token = crate::support::host_token(systemprompt_models::bridge::host::HostKind::OpenCode);
    assert_ne!(shown, &token, "the raw host token must not reach the GUI");
}

#[test]
fn an_mdm_provider_reads_as_absent_until_the_token_is_written() {
    let snapshot = sandbox(Some(MDM_BLOCK), |_| OPENCODE_HOST.probe(&probe_env()));
    assert!(
        !matches!(snapshot.profile_state, ProfileState::Installed),
        "{:?}",
        snapshot.profile_state
    );
}

#[test]
fn an_mdm_provider_with_the_token_written_is_installed() {
    let snapshot = sandbox(Some(MDM_BLOCK), |_| {
        install();
        OPENCODE_HOST.probe(&probe_env())
    });
    assert!(
        matches!(snapshot.profile_state, ProfileState::Installed),
        "{:?}",
        snapshot.profile_state
    );
    assert!(
        snapshot
            .profile_source
            .as_deref()
            .is_some_and(|p| p.contains("managed")),
        "{:?}",
        snapshot.profile_source
    );
}

#[test]
fn nothing_anywhere_is_absent() {
    let snapshot = sandbox(None, |_| OPENCODE_HOST.probe(&probe_env()));
    assert!(matches!(snapshot.profile_state, ProfileState::Absent));
}

#[test]
fn removal_treats_an_identical_managed_and_user_path_as_one_tier() {
    let root = TempDir::new().expect("sandbox");
    let config_home = root.path().join("config");
    let shared = config_home.join("opencode");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(
        shared.join("opencode.json"),
        r#"{
          "operator": {"retain": true},
          "providers": {"systemprompt": {
            "package": "aisdk:@ai-sdk/openai-compatible",
            "settings": {"baseURL": "http://127.0.0.1:48217/v1"}
          }}
        }"#,
    )
    .unwrap();
    write_bridge_config(&config_home, &shared);
    let vars = [
        ("HOME", Some(root.path().display().to_string())),
        ("USERPROFILE", Some(root.path().display().to_string())),
        ("XDG_CONFIG_HOME", Some(config_home.display().to_string())),
        ("SP_BRIDGE_CONFIG", None),
    ];

    let removal = temp_env::with_vars(vars, || OPENCODE_HOST.remove_profile())
        .expect("the single physical tier is cleaned once");
    assert!(matches!(removal, ProfileRemoval::Removed { .. }));
    let retained: serde_json::Value =
        serde_json::from_slice(&std::fs::read(shared.join("opencode.json")).unwrap()).unwrap();
    assert_eq!(retained, serde_json::json!({"operator": {"retain": true}}));
}
