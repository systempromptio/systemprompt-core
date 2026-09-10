use systemprompt_bridge::gateway::manifest::AutoUpdatePolicy;
use systemprompt_bridge::sync::read_last_sync;
use tempfile::TempDir;

fn sentinel(body: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("last-sync.json");
    std::fs::write(&path, body).expect("seed sentinel");
    (dir, path)
}

#[test]
fn a_sentinel_written_before_this_release_stages_updates() {
    let (_dir, path) =
        sentinel(r#"{"last_applied_manifest_version":"2026-09-10T00:00:00Z-abcdef01"}"#);

    let state = read_last_sync(&path)
        .expect("read")
        .expect("sentinel present");

    assert_eq!(
        state.auto_update,
        AutoUpdatePolicy::Staged,
        "a bridge that synced against an older gateway must keep updating itself"
    );
}

#[test]
fn a_gateway_that_disables_updates_is_carried_on_the_sentinel() {
    let (_dir, path) = sentinel(
        r#"{"last_applied_manifest_version":"2026-09-10T00:00:00Z-abcdef01","auto_update":"disabled"}"#,
    );

    let state = read_last_sync(&path)
        .expect("read")
        .expect("sentinel present");

    assert_eq!(state.auto_update, AutoUpdatePolicy::Disabled);
    assert!(!state.auto_update.stages());
}

#[test]
fn the_policy_wire_form_is_the_yaml_operators_write() {
    let disabled: AutoUpdatePolicy = serde_json::from_str("\"disabled\"").expect("disabled");
    let staged: AutoUpdatePolicy = serde_json::from_str("\"staged\"").expect("staged");

    assert_eq!(disabled, AutoUpdatePolicy::Disabled);
    assert_eq!(staged, AutoUpdatePolicy::Staged);
    assert!(staged.stages());
}

#[test]
fn a_corrupt_sentinel_is_reported_rather_than_defaulted() {
    let (_dir, path) = sentinel("{not json");

    let err = read_last_sync(&path).expect_err("corrupt sentinel must not read as a default");

    assert!(format!("{err}").contains("parse replay state"), "{err}");
}
