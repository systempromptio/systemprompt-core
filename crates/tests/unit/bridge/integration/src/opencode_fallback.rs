//! The Linux user-tier fallback: where `/etc/opencode` cannot be written and
//! there is no elevation to offer, `install` puts the provider block in the
//! user config instead. A probe that ignored that tier would report a working
//! host as unconfigured.

use std::path::Path;

use systemprompt_bridge::integration::host_app::{HostApp, ProbeEnv, ProfileState};
use systemprompt_bridge::integration::opencode::OPENCODE_HOST;
use tempfile::TempDir;

fn probe_env() -> ProbeEnv {
    ProbeEnv {
        proxy_port: systemprompt_bridge::proxy::DEFAULT_PROXY_PORT,
        loopback_secret_fingerprint: None,
        start_menu: std::sync::Arc::default(),
    }
}

const PROVIDER_BLOCK: &str = r#"{
  "provider": {
    "systemprompt": {
      "npm": "@ai-sdk/openai-compatible",
      "options": { "baseURL": "http://127.0.0.1:48217/v1" }
    }
  }
}"#;

/// Seeds the managed tier and/or the user tier, then probes.
fn sandbox<R>(managed: Option<&str>, user: Option<&str>, f: impl FnOnce(&Path) -> R) -> R {
    let root = TempDir::new().expect("sandbox");
    let managed_dir = root.path().join("managed");
    std::fs::create_dir_all(&managed_dir).expect("managed dir");
    if let Some(body) = managed {
        std::fs::write(managed_dir.join("opencode.json"), body).expect("seed managed config");
    }
    let config_home = root.path().join("config");
    if let Some(body) = user {
        let dir = config_home.join("opencode");
        std::fs::create_dir_all(&dir).expect("user dir");
        std::fs::write(dir.join("opencode.json"), body).expect("seed user config");
    }
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(root.path().display().to_string())),
        ("XDG_CONFIG_HOME", Some(config_home.display().to_string())),
        (
            "XDG_DATA_HOME",
            Some(root.path().join("data").display().to_string()),
        ),
        (
            "SP_BRIDGE_OPENCODE_MANAGED_DIR",
            Some(managed_dir.display().to_string()),
        ),
        ("PATH", Some(root.path().join("bin").display().to_string())),
    ];
    let out = temp_env::with_vars(vars, || f(root.path()));
    drop(root);
    out
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn a_user_tier_provider_block_probes_as_installed_when_the_managed_tier_is_empty() {
    let snapshot = sandbox(None, Some(PROVIDER_BLOCK), |_| {
        OPENCODE_HOST.probe(&probe_env())
    });
    assert!(
        matches!(snapshot.profile_state, ProfileState::Installed),
        "the fallback tier must count as installed, got {:?}",
        snapshot.profile_state
    );
    assert!(
        snapshot
            .profile_source
            .as_deref()
            .is_some_and(|p| p.contains("config")),
        "the source must name the user tier so the operator sees which tier answered: {:?}",
        snapshot.profile_source
    );
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn the_managed_tier_still_wins_over_the_user_tier() {
    let stale = r#"{
      "provider": {
        "systemprompt": {
          "npm": "@ai-sdk/openai-compatible",
          "options": { "baseURL": "http://127.0.0.1:1/v1" }
        }
      }
    }"#;
    let snapshot = sandbox(Some(stale), Some(PROVIDER_BLOCK), |_| {
        OPENCODE_HOST.probe(&probe_env())
    });
    assert!(
        matches!(snapshot.profile_state, ProfileState::Stale { .. }),
        "a managed file on the wrong port is stale however good the user tier is, got {:?}",
        snapshot.profile_state
    );
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[test]
fn the_user_tier_is_not_read_where_elevation_exists() {
    let snapshot = sandbox(None, Some(PROVIDER_BLOCK), |_| {
        OPENCODE_HOST.probe(&probe_env())
    });
    assert!(
        matches!(snapshot.profile_state, ProfileState::Absent),
        "macOS and Windows can prompt for elevation, so a user-tier block is not governance and \
         must not mask a missing managed tier: {:?}",
        snapshot.profile_state
    );
}
