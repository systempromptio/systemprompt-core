//! The automatic-update decision is fail-closed on the last-sync sentinel: a
//! sentinel that cannot be read, or that another gateway wrote, withholds
//! staging rather than falling back to the policy default.

use systemprompt_bridge::update::{AutoUpdateDecision, auto_update_policy, automatic_enabled};
use systemprompt_identifiers::ValidatedUrl;
use tempfile::TempDir;

fn in_sandbox<R>(state: &TempDir, f: impl FnOnce() -> R) -> R {
    let root = state.path().display().to_string();
    temp_env::with_vars(
        vec![
            ("HOME", Some(root.clone())),
            ("XDG_STATE_HOME", Some(root.clone())),
            ("XDG_CONFIG_HOME", Some(root)),
            ("SP_BRIDGE_CONFIG", None),
        ],
        f,
    )
}

fn seed_config(state: &TempDir, gateway: &str) {
    let config_dir = state.path().join("systemprompt");
    std::fs::create_dir_all(&config_dir).expect("config dir");
    std::fs::write(
        config_dir.join("systemprompt-bridge.toml"),
        format!("gateway_url = \"{gateway}\"\n"),
    )
    .expect("seed config");
}

fn seed_sentinel(state: &TempDir, body: &str) {
    let meta = state.path().join("systemprompt-bridge").join("metadata");
    std::fs::create_dir_all(&meta).expect("metadata dir");
    std::fs::write(meta.join("last-sync.json"), body).expect("seed last-sync");
}

#[test]
fn a_corrupt_sentinel_withholds_staging_and_names_the_read_error() {
    let state = TempDir::new().expect("state");
    seed_config(&state, "https://gateway.example.com");
    seed_sentinel(&state, "{ not json");

    let decision = in_sandbox(&state, auto_update_policy);
    assert!(
        matches!(decision, AutoUpdateDecision::Withheld(_)),
        "{}",
        decision.describe()
    );
    assert!(!decision.stages());
    assert!(
        decision.describe().contains("unreadable"),
        "{}",
        decision.describe()
    );
    assert!(!in_sandbox(&state, automatic_enabled));
}

#[test]
fn a_sentinel_written_by_another_gateway_does_not_deliver_its_policy() {
    let state = TempDir::new().expect("state");
    seed_config(&state, "https://gateway.example.com");
    let other = ValidatedUrl::try_new("https://other.example.com").expect("url");
    seed_sentinel(
        &state,
        &serde_json::json!({ "gateway": other, "auto_update": "staged" }).to_string(),
    );

    let decision = in_sandbox(&state, auto_update_policy);
    assert!(
        matches!(decision, AutoUpdateDecision::NeverSynced),
        "a policy delivered by a different gateway is not this gateway's policy: {}",
        decision.describe()
    );
    assert!(!in_sandbox(&state, automatic_enabled));
}

#[test]
fn a_sentinel_from_the_configured_gateway_delivers_its_policy() {
    let state = TempDir::new().expect("state");
    seed_config(&state, "https://gateway.example.com");
    let gateway = ValidatedUrl::try_new("https://gateway.example.com").expect("url");
    seed_sentinel(
        &state,
        &serde_json::json!({ "gateway": gateway, "auto_update": "staged" }).to_string(),
    );

    let decision = in_sandbox(&state, auto_update_policy);
    assert!(
        matches!(decision, AutoUpdateDecision::Delivered(_)),
        "{}",
        decision.describe()
    );
    assert!(in_sandbox(&state, automatic_enabled));
}
