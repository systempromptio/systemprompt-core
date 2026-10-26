//! Installed-plugin stamps: a plugin is skipped only when its stamp names the
//! manifest's digest for the same org-plugins root.

use systemprompt_bridge::gateway::manifest::PluginEntry;
use systemprompt_bridge::ids::{PluginId, Sha256Digest};
use systemprompt_bridge::sync::apply::stamp::{self, PluginStamp};

fn plugin(sha: char) -> PluginEntry {
    PluginEntry {
        id: PluginId::try_new("acme-plugin").unwrap(),
        version: "1.0.0".into(),
        sha256: Sha256Digest::try_new(sha.to_string().repeat(64)).unwrap(),
        files: vec![],
        hooks: systemprompt_models::plugin::PluginHooksRef::default(),
        publication: None,
    }
}

fn stamped(meta: &std::path::Path, root: &std::path::Path, sha: char) {
    stamp::write(
        meta,
        &PluginId::try_new("acme-plugin").unwrap(),
        &PluginStamp {
            sha256: sha.to_string().repeat(64),
            root: root.to_path_buf(),
            mcp_servers: vec!["github".to_owned()],
        },
    )
    .unwrap();
}

#[test]
fn a_matching_stamp_over_an_installed_dir_is_current() {
    let meta = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("acme-plugin")).unwrap();
    stamped(meta.path(), root.path(), 'a');
    let current = stamp::current(meta.path(), root.path(), &plugin('a')).expect("skip");
    assert_eq!(current.mcp_servers, vec!["github".to_owned()]);
}

#[test]
fn a_changed_digest_is_not_current() {
    let meta = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("acme-plugin")).unwrap();
    stamped(meta.path(), root.path(), 'a');
    assert!(stamp::current(meta.path(), root.path(), &plugin('b')).is_none());
}

#[test]
fn a_missing_plugin_dir_is_not_current() {
    let meta = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    stamped(meta.path(), root.path(), 'a');
    assert!(stamp::current(meta.path(), root.path(), &plugin('a')).is_none());
}

#[test]
fn a_stamp_for_another_root_is_not_current() {
    let meta = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("acme-plugin")).unwrap();
    stamped(meta.path(), other.path(), 'a');
    assert!(stamp::current(meta.path(), root.path(), &plugin('a')).is_none());
}

#[test]
fn a_cleared_stamp_is_not_current() {
    let meta = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("acme-plugin")).unwrap();
    stamped(meta.path(), root.path(), 'a');
    stamp::clear(meta.path(), &PluginId::try_new("acme-plugin").unwrap()).unwrap();
    stamp::clear(meta.path(), &PluginId::try_new("acme-plugin").unwrap()).unwrap();
    assert!(stamp::current(meta.path(), root.path(), &plugin('a')).is_none());
}
