//! Host targeting survives import: a rule's frontmatter `hosts` and a
//! plugin's `hosts/` tree.

use std::path::Path;

use systemprompt_manifest::services::DiskRuleConfig;
use systemprompt_marketplace::{ImportOptions, import_anthropic_tree};
use tempfile::TempDir;

fn tree(rule_front: &str) -> TempDir {
    let root = TempDir::new().expect("tempdir");
    let write = |rel: &str, body: &str| {
        let path = root.path().join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, body).expect("write");
    };
    write(
        ".claude-plugin/marketplace.json",
        r#"{"name":"acme","owner":{"name":"Acme"},"plugins":[{"name":"alpha"}]}"#,
    );
    write(
        "plugins/alpha/.claude-plugin/plugin.json",
        r#"{"name":"alpha"}"#,
    );
    write(
        "plugins/alpha/skills/a-skill/SKILL.md",
        "---\nname: a-skill\ndescription: What it does.\n---\n\nBody.\n",
    );
    write(
        "plugins/alpha/rules/house-style.md",
        &format!("---\n{rule_front}---\n\nWrite plainly.\n"),
    );
    write(
        "plugins/alpha/hosts/opencode/themes/acme.json",
        r#"{"theme":{}}"#,
    );
    root
}

fn import(source: &Path) -> Result<TempDir, String> {
    let dest = TempDir::new().expect("tempdir");
    import_anthropic_tree(
        source,
        dest.path(),
        &ImportOptions::new(std::env::temp_dir()),
    )
    .map(|_| dest)
    .map_err(|e| e.to_string())
}

fn rule_config(dest: &Path) -> DiskRuleConfig {
    let text = std::fs::read_to_string(dest.join("rules/house_style/config.yaml"))
        .expect("rule config written");
    serde_yaml::from_str(&text).expect("rule config parses")
}

#[test]
fn rule_frontmatter_hosts_reach_the_rule_config() {
    let source = tree("name: House style\nhosts: [opencode, opencode-cli]\n");
    let dest = import(source.path()).expect("import succeeds");

    assert_eq!(
        rule_config(dest.path()).hosts,
        vec!["opencode", "opencode-cli"]
    );
}

#[test]
fn a_rule_without_hosts_targets_every_host() {
    let source = tree("name: House style\n");
    let dest = import(source.path()).expect("import succeeds");

    assert!(rule_config(dest.path()).hosts.is_empty());
}

#[test]
fn an_unknown_rule_host_is_refused() {
    let source = tree("hosts: [not-a-host]\n");
    let err = import(source.path()).expect_err("unknown host refused");

    assert!(err.contains("rule frontmatter `hosts`"), "{err}");
}

#[test]
fn the_plugin_hosts_tree_is_copied_beside_the_plugin_config() {
    let source = tree("name: House style\n");
    let dest = import(source.path()).expect("import succeeds");

    let theme = dest
        .path()
        .join("plugins/alpha/hosts/opencode/themes/acme.json");
    assert_eq!(
        std::fs::read_to_string(theme).expect("theme copied"),
        r#"{"theme":{}}"#
    );
}
