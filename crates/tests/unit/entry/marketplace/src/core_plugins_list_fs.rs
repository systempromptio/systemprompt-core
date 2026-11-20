//! Filesystem-driven tests for `core plugins list` scanning.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use systemprompt_cli::core::plugins::list::scan_plugins;

const PLUGIN_YAML: &str = r#"
plugin:
  id: {id}
  name: {id} plugin
  description: A demo plugin
  version: 1.0.0
  author:
    name: Tester
    email: tester@example.com
  keywords: [demo]
  license: MIT
  category: tools
  enabled: {enabled}
  skills:
    source: explicit
    include: [alpha, beta]
  agents:
    source: instance
"#;

fn write_plugin(root: &Path, id: &str, enabled: bool) {
    let dir = root.join(id);
    fs::create_dir_all(&dir).unwrap();
    let yaml = PLUGIN_YAML
        .replace("{id}", id)
        .replace("{enabled}", if enabled { "true" } else { "false" });
    fs::write(dir.join("config.yaml"), yaml).unwrap();
}

#[test]
fn missing_plugins_dir_yields_empty_list() {
    assert!(
        scan_plugins(Path::new("/nonexistent/plugins-root"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn scan_sorts_by_id_and_counts_explicit_components_only() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(tmp.path(), "zeta", true);
    write_plugin(tmp.path(), "alpha", false);

    let plugins = scan_plugins(tmp.path()).unwrap();
    assert_eq!(plugins.len(), 2);
    assert_eq!(plugins[0].id.as_str(), "alpha");
    assert_eq!(plugins[1].id.as_str(), "zeta");
    assert!(!plugins[0].enabled);
    assert!(plugins[1].enabled);
    assert_eq!(plugins[0].skill_count, 2);
    assert_eq!(plugins[0].agent_count, 0);
}

#[test]
fn scan_skips_non_dirs_missing_configs_and_bad_yaml() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(tmp.path(), "good", true);
    fs::write(tmp.path().join("stray.txt"), "x").unwrap();
    fs::create_dir_all(tmp.path().join("no-config")).unwrap();
    let bad = tmp.path().join("bad");
    fs::create_dir_all(&bad).unwrap();
    fs::write(bad.join("config.yaml"), "plugin: [nope").unwrap();

    let plugins = scan_plugins(tmp.path()).unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].id.as_str(), "good");
}
