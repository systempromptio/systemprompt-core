use systemprompt_bridge::cli::doctor::Status;
use systemprompt_bridge::cli::doctor::filesystem::{
    check_bridge_working_dir, check_org_plugins_writable,
};
use tempfile::TempDir;

fn with_dirs<R>(data: &TempDir, state: &TempDir, home: &TempDir, f: impl FnOnce() -> R) -> R {
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(home.path().display().to_string())),
        ("XDG_CONFIG_HOME", Some(home.path().display().to_string())),
        ("XDG_DATA_HOME", Some(data.path().display().to_string())),
        ("XDG_STATE_HOME", Some(state.path().display().to_string())),
    ];
    temp_env::with_vars(vars, f)
}

#[test]
fn the_working_dir_check_creates_and_probes_staging_and_metadata() {
    let (data, state, home) = (
        TempDir::new().expect("data"),
        TempDir::new().expect("state"),
        TempDir::new().expect("home"),
    );
    let check = with_dirs(&data, &state, &home, check_bridge_working_dir);
    assert_eq!(check.status, Status::Ok, "{}", check.detail);
    assert!(
        state
            .path()
            .join("systemprompt-bridge")
            .join("staging")
            .is_dir(),
        "the check materialises the staging dir"
    );
    assert!(
        state
            .path()
            .join("systemprompt-bridge")
            .join("metadata")
            .is_dir(),
        "the check materialises the metadata dir"
    );
    assert!(
        !state
            .path()
            .join("systemprompt-bridge")
            .join("metadata")
            .join(".sp-bridge-writeprobe")
            .exists(),
        "the write probe is cleaned up"
    );
}

#[test]
fn the_org_plugins_check_warns_before_install_and_passes_after() {
    let (data, state, home) = (
        TempDir::new().expect("data"),
        TempDir::new().expect("state"),
        TempDir::new().expect("home"),
    );
    let before = with_dirs(&data, &state, &home, check_org_plugins_writable);
    assert_eq!(before.status, Status::Warn, "{}", before.detail);
    assert!(before.detail.contains("not present"), "{}", before.detail);

    std::fs::create_dir_all(data.path().join("Claude").join("org-plugins"))
        .expect("org plugins root");
    let after = with_dirs(&data, &state, &home, check_org_plugins_writable);
    assert_eq!(after.status, Status::Ok, "{}", after.detail);
    assert!(after.detail.contains("writable"), "{}", after.detail);
    assert!(
        !data
            .path()
            .join("Claude")
            .join("org-plugins")
            .join(".sp-bridge-writeprobe")
            .exists(),
        "the write probe is cleaned up"
    );
}
