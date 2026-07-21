//! `apply_enable` must reconcile Cowork's materialised copy of the
//! org-provisioned marketplace, not just the `enabledPlugins` map: Cowork
//! installs each plugin into its own tree and never removes one the manifest
//! has dropped.

use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_bridge::integration::cowork_plugins::{CoworkTarget, apply_enable};
use tempfile::tempdir;

const MARKETPLACE: &str = "org-provisioned";

fn target_for(session_org_dir: &Path) -> CoworkTarget {
    CoworkTarget {
        session_org_dir: session_org_dir.to_path_buf(),
        cowork_plugins_dir: session_org_dir.join("cowork_plugins"),
    }
}

fn marketplace_dir(target: &CoworkTarget) -> PathBuf {
    target
        .cowork_plugins_dir
        .join("marketplaces")
        .join(MARKETPLACE)
}

fn install_plugin(target: &CoworkTarget, name: &str) {
    let dir = marketplace_dir(target).join(name).join("skills");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("SKILL.md"), "# skill").unwrap();
    let cache = target
        .cowork_plugins_dir
        .join("cache")
        .join(MARKETPLACE)
        .join(name);
    fs::create_dir_all(&cache).unwrap();
}

fn write_installed(target: &CoworkTarget, names: &[&str]) {
    let plugins: serde_json::Map<String, serde_json::Value> = names
        .iter()
        .map(|n| {
            (
                format!("{n}@{MARKETPLACE}"),
                serde_json::json!([{ "scope": "user", "source": "local" }]),
            )
        })
        .collect();
    let body = serde_json::json!({ "version": 2, "plugins": plugins });
    fs::write(
        target.cowork_plugins_dir.join("installed_plugins.json"),
        serde_json::to_vec_pretty(&body).unwrap(),
    )
    .unwrap();
}

fn installed_keys(target: &CoworkTarget) -> Vec<String> {
    let text =
        fs::read_to_string(target.cowork_plugins_dir.join("installed_plugins.json")).unwrap();
    let root: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut keys: Vec<String> = root["plugins"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

fn setup(plugins: &[&str]) -> (tempfile::TempDir, CoworkTarget) {
    let temp = tempdir().unwrap();
    let target = target_for(temp.path());
    fs::create_dir_all(&target.cowork_plugins_dir).unwrap();
    for name in plugins {
        install_plugin(&target, name);
    }
    write_installed(&target, plugins);
    (temp, target)
}

#[test]
fn removes_plugin_absent_from_the_manifest() {
    let (_temp, target) = setup(&["astound-admin", "systemprompt-managed"]);

    apply_enable(&target, &["astound-admin"]).unwrap();

    assert!(marketplace_dir(&target).join("astound-admin").is_dir());
    assert!(
        !marketplace_dir(&target)
            .join("systemprompt-managed")
            .exists()
    );
    assert!(
        !target
            .cowork_plugins_dir
            .join("cache")
            .join(MARKETPLACE)
            .join("systemprompt-managed")
            .exists()
    );
    assert_eq!(
        installed_keys(&target),
        vec![format!("astound-admin@{MARKETPLACE}")]
    );
}

#[test]
fn keeps_every_manifest_plugin() {
    let (_temp, target) = setup(&["astound-admin", "astound-commons"]);

    apply_enable(&target, &["astound-admin", "astound-commons"]).unwrap();

    assert!(marketplace_dir(&target).join("astound-admin").is_dir());
    assert!(marketplace_dir(&target).join("astound-commons").is_dir());
    assert_eq!(
        installed_keys(&target),
        vec![
            format!("astound-admin@{MARKETPLACE}"),
            format!("astound-commons@{MARKETPLACE}"),
        ]
    );
}

#[test]
fn ignores_dot_prefixed_entries() {
    let (_temp, target) = setup(&["astound-admin"]);
    fs::create_dir_all(marketplace_dir(&target).join(".metadata")).unwrap();

    apply_enable(&target, &["astound-admin"]).unwrap();

    assert!(marketplace_dir(&target).join(".metadata").is_dir());
}
