use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use systemprompt_identifiers::{PluginId, SkillId};
use systemprompt_marketplace::bundle::{BundleContent, build_plugin_bundle};
use systemprompt_marketplace::managed::{AssetFile, RevisionFiles};
use systemprompt_models::plugin::PluginComponentRef;

use crate::bundle::plugin_config;

static NO_DISABLED: BTreeSet<String> = BTreeSet::new();

fn content<'a>(
    root: &'a Path,
    managed: &'a BTreeMap<PluginId, RevisionFiles>,
    files: &'a BTreeMap<SkillId, RevisionFiles>,
) -> BundleContent<'a> {
    BundleContent {
        skills: &[],
        rules: &[],
        agents: &[],
        mcp_servers: &[],
        disabled_mcp_servers: &NO_DISABLED,
        artifacts: &[],
        plugins_root: root,
        managed_files: files,
        managed_plugin_files: managed,
    }
}

fn write(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, body).expect("write");
}

#[test]
fn disk_host_files_travel_with_the_bundle_minus_dev_dirs_and_links() {
    let root = tempfile::tempdir().expect("tempdir");
    let plugin = root.path().join("hosted");
    write(
        &plugin.join("hosts/opencode/plugin.js"),
        "export default {}",
    );
    write(
        &plugin.join("hosts/opencode/lib/util.js"),
        "export const x = 1",
    );
    write(&plugin.join("hosts/opencode/tests/plugin.js"), "dev only");
    write(&plugin.join("outside.txt"), "not a host file");
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        plugin.join("outside.txt"),
        plugin.join("hosts/opencode/link.txt"),
    )
    .expect("symlink");

    let managed = BTreeMap::new();
    let files = BTreeMap::new();
    let config = plugin_config(
        "hosted",
        PluginComponentRef::default(),
        PluginComponentRef::default(),
    );
    let bundle =
        build_plugin_bundle(&config, &content(root.path(), &managed, &files)).expect("build");

    let hosts: Vec<&str> = bundle
        .keys()
        .map(String::as_str)
        .filter(|k| k.starts_with("hosts/"))
        .collect();
    assert_eq!(
        hosts,
        ["hosts/opencode/lib/util.js", "hosts/opencode/plugin.js"]
    );
    let file = &bundle["hosts/opencode/plugin.js"];
    assert_eq!(file.bytes, b"export default {}");
    assert!(!file.executable);
}

#[test]
fn a_disk_plugin_without_hosts_contributes_no_host_files() {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(root.path().join("bare")).expect("mkdir");
    let managed = BTreeMap::new();
    let files = BTreeMap::new();
    let config = plugin_config(
        "bare",
        PluginComponentRef::default(),
        PluginComponentRef::default(),
    );
    let bundle =
        build_plugin_bundle(&config, &content(root.path(), &managed, &files)).expect("build");
    assert!(!bundle.keys().any(|k| k.starts_with("hosts/")));
}

fn asset(body: &str, executable: bool) -> AssetFile {
    AssetFile {
        bytes: body.as_bytes().to_vec(),
        media_type: "text/plain".to_owned(),
        executable,
    }
}

#[test]
fn managed_host_files_come_from_the_revision_with_their_mode() {
    let root = tempfile::tempdir().expect("tempdir");
    write(
        &root.path().join("pub/hosts/opencode/disk.js"),
        "ignored: the revision wins",
    );
    let mut revision = BTreeMap::new();
    revision.insert("hosts/opencode/run.sh".to_owned(), asset("#!/bin/sh", true));
    revision.insert("hosts/opencode/tests/x.js".to_owned(), asset("dev", false));
    revision.insert("hostsish/other.js".to_owned(), asset("no", false));
    let managed = BTreeMap::from([(PluginId::new("pub"), RevisionFiles(revision))]);
    let files = BTreeMap::new();
    let config = plugin_config(
        "pub",
        PluginComponentRef::default(),
        PluginComponentRef::default(),
    );
    let bundle =
        build_plugin_bundle(&config, &content(root.path(), &managed, &files)).expect("build");

    let hosts: Vec<&str> = bundle
        .keys()
        .map(String::as_str)
        .filter(|k| k.starts_with("hosts"))
        .collect();
    assert_eq!(hosts, ["hosts/opencode/run.sh"]);
    assert!(bundle["hosts/opencode/run.sh"].executable);
}
