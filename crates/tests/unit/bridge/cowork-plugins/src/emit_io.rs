//! IO tests for the Cowork emit layer: `resolve_target` against a sandboxed
//! `XDG_CONFIG_HOME` session tree, and `apply_enable`/`clear_all` including the
//! legacy session-marketplace purge.

// `PLUGIN` doubles as a synced plugin id and, being the legacy aggregate name,
// as the target of the legacy-state purge.

use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_bridge::integration::cowork_plugins::{
    CoworkTarget, PERSONAL_SESSION_UUID, apply_enable, clear_all, resolve_target,
};
use tempfile::tempdir;

const PLUGIN: &str = "systemprompt-managed";
const LEGACY_MP: &str = "systemprompt-bridge-managed";

fn session_tree(config_home: &Path, account: &str, org: &str) -> PathBuf {
    let dir = config_home
        .join("Claude-3p")
        .join("local-agent-mode-sessions")
        .join(account)
        .join(org);
    fs::create_dir_all(dir.join("cowork_plugins")).unwrap();
    dir
}

fn target_for(session_org_dir: &Path) -> CoworkTarget {
    CoworkTarget {
        session_org_dir: session_org_dir.to_path_buf(),
        cowork_plugins_dir: session_org_dir.join("cowork_plugins"),
    }
}

fn settings(session_org_dir: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(session_org_dir.join("cowork_settings.json")).unwrap())
        .unwrap()
}

#[test]
fn resolve_target_none_when_sessions_root_missing() {
    let temp = tempdir().unwrap();
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        assert!(resolve_target().is_none());
    });
}

#[test]
fn resolve_target_prefers_personal_session_dir() {
    let temp = tempdir().unwrap();
    let personal = session_tree(temp.path(), "acct-1", PERSONAL_SESSION_UUID);
    let _other = session_tree(
        temp.path(),
        "acct-1",
        "11111111-2222-4333-8444-555566667777",
    );

    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let target = resolve_target().expect("personal session should resolve");
        assert_eq!(target.session_org_dir, personal);
        assert_eq!(target.cowork_plugins_dir, personal.join("cowork_plugins"));
    });
}

#[test]
fn resolve_target_falls_back_to_mtime_without_personal_dir() {
    let temp = tempdir().unwrap();
    let only = session_tree(
        temp.path(),
        "acct-1",
        "11111111-2222-4333-8444-555566667777",
    );

    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let target = resolve_target().expect("fallback should resolve");
        assert_eq!(target.session_org_dir, only);
    });
}

#[test]
fn apply_enable_writes_enabled_key_and_purges_legacy_state() {
    let temp = tempdir().unwrap();
    let org = temp.path().join("org");
    fs::create_dir_all(org.join("cowork_plugins")).unwrap();
    let target = target_for(&org);

    let plugins = &target.cowork_plugins_dir;
    fs::create_dir_all(plugins.join("marketplaces").join(LEGACY_MP)).unwrap();
    fs::create_dir_all(plugins.join("cache").join(LEGACY_MP)).unwrap();
    fs::write(
        plugins.join("installed_plugins.json"),
        serde_json::to_vec(&serde_json::json!({
            "plugins": {
                (format!("{PLUGIN}@{LEGACY_MP}")): [{ "scope": "user" }],
                "user-plugin@user-mp": [{ "scope": "user" }],
            }
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        plugins.join("known_marketplaces.json"),
        serde_json::to_vec(&serde_json::json!({
            (LEGACY_MP): { "source": "legacy" },
            "user-mp": { "source": "keep" },
        }))
        .unwrap(),
    )
    .unwrap();

    let report = apply_enable(&target, &[PLUGIN]).unwrap();
    assert!(report.enabled);
    assert_eq!(report.target.as_deref(), Some(org.as_path()));

    let s = settings(&org);
    assert_eq!(
        s["enabledPlugins"][format!("{PLUGIN}@org-provisioned")],
        true
    );

    assert!(!plugins.join("marketplaces").join(LEGACY_MP).exists());
    assert!(!plugins.join("cache").join(LEGACY_MP).exists());

    let installed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(plugins.join("installed_plugins.json")).unwrap())
            .unwrap();
    assert!(installed["plugins"][format!("{PLUGIN}@{LEGACY_MP}")].is_null());
    assert!(!installed["plugins"]["user-plugin@user-mp"].is_null());

    let known: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(plugins.join("known_marketplaces.json")).unwrap())
            .unwrap();
    assert!(known[LEGACY_MP].is_null());
    assert_eq!(known["user-mp"]["source"], "keep");
}

#[test]
fn apply_enable_without_legacy_state_is_clean() {
    let temp = tempdir().unwrap();
    let org = temp.path().join("org");
    fs::create_dir_all(org.join("cowork_plugins")).unwrap();
    let target = target_for(&org);

    let report = apply_enable(&target, &[PLUGIN]).unwrap();
    assert!(report.enabled);
    assert_eq!(
        settings(&org)["enabledPlugins"][format!("{PLUGIN}@org-provisioned")],
        true
    );
}

#[test]
fn apply_enable_reconciles_to_the_current_plugin_set() {
    let temp = tempdir().unwrap();
    let org = temp.path().join("org");
    fs::create_dir_all(org.join("cowork_plugins")).unwrap();
    let target = target_for(&org);

    apply_enable(&target, &["alpha", "beta"]).unwrap();
    apply_enable(&target, &["beta", "gamma"]).unwrap();

    let s = settings(&org);
    assert!(
        s["enabledPlugins"]["alpha@org-provisioned"].is_null(),
        "a plugin dropped from the manifest must lose its enable key"
    );
    assert_eq!(s["enabledPlugins"]["beta@org-provisioned"], true);
    assert_eq!(s["enabledPlugins"]["gamma@org-provisioned"], true);
}

#[test]
fn clear_all_removes_enabled_key_and_preserves_foreign_keys() {
    let temp = tempdir().unwrap();
    let org = temp.path().join("org");
    fs::create_dir_all(org.join("cowork_plugins")).unwrap();
    let target = target_for(&org);

    fs::write(
        org.join("cowork_settings.json"),
        serde_json::to_vec(&serde_json::json!({
            "enabledPlugins": {
                (format!("{PLUGIN}@org-provisioned")): true,
                "user-plugin@user-mp": true,
            },
            "theme": "dark",
        }))
        .unwrap(),
    )
    .unwrap();

    clear_all(&target).unwrap();

    let s = settings(&org);
    assert!(s["enabledPlugins"][format!("{PLUGIN}@org-provisioned")].is_null());
    assert_eq!(s["enabledPlugins"]["user-plugin@user-mp"], true);
    assert_eq!(s["theme"], "dark");
}

#[test]
fn clear_all_without_settings_file_is_noop() {
    let temp = tempdir().unwrap();
    let org = temp.path().join("org");
    fs::create_dir_all(org.join("cowork_plugins")).unwrap();
    clear_all(&target_for(&org)).unwrap();
    assert!(!org.join("cowork_settings.json").exists());
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(unix)]
#[test]
fn an_unremovable_legacy_marketplace_dir_fails_the_purge_with_its_path() {
    let temp = tempdir().unwrap();
    let org = temp.path().join("org");
    let marketplaces = org.join("cowork_plugins").join("marketplaces");
    let legacy = marketplaces.join(LEGACY_MP);
    fs::create_dir_all(&legacy).unwrap();
    set_mode(&marketplaces, 0o555);

    let err = apply_enable(&target_for(&org), &[PLUGIN])
        .expect_err("an unremovable legacy dir must fail the apply");
    set_mode(&marketplaces, 0o755);

    let msg = err.to_string();
    assert!(msg.contains("remove_dir_all"), "{msg}");
    assert!(msg.contains(LEGACY_MP), "{msg}");
    assert!(legacy.is_dir(), "the legacy dir survives the failed purge");
}

#[cfg(unix)]
#[test]
fn an_unwritable_known_marketplaces_file_fails_the_purge_at_the_atomic_write() {
    let temp = tempdir().unwrap();
    let org = temp.path().join("org");
    let plugins_dir = org.join("cowork_plugins");
    fs::create_dir_all(&plugins_dir).unwrap();
    fs::write(
        plugins_dir.join("known_marketplaces.json"),
        format!(r#"{{"{LEGACY_MP}": {{"source": "legacy"}}}}"#),
    )
    .unwrap();
    set_mode(&plugins_dir, 0o555);

    let err = apply_enable(&target_for(&org), &[PLUGIN])
        .expect_err("an unwritable state file must fail the apply");
    set_mode(&plugins_dir, 0o755);

    let msg = err.to_string();
    assert!(msg.contains("atomic_write"), "{msg}");
    assert!(msg.contains("known_marketplaces.json"), "{msg}");
    let kept: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(plugins_dir.join("known_marketplaces.json")).unwrap(),
    )
    .unwrap();
    assert!(
        kept.get(LEGACY_MP).is_some(),
        "the legacy entry survives the failed write: {kept}"
    );
}
