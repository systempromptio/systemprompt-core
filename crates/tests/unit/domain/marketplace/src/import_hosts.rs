//! Host targeting survives import: a rule's frontmatter `hosts`, a plugin's
//! `hosts/` tree without its dev files, and the `OpenCode` themes a
//! marketplace pins.

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

const THEME_SHA: &str = "b2b9b2ed6c1f09a0ef7efb3f0d49a7f1a0e8d6b45ae7e13f1d4d1e1a1cd63c3b";

fn write(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, body).expect("write");
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn with_opencode(source: &Path, sha: &str) {
    write(
        source,
        ".claude-plugin/systemprompt.yaml",
        &format!(
            "schema: 1\nmarketplace:\n  opencode:\n    themes:\n      - name: acme\n        sha256: {sha}\n"
        ),
    );
}

fn import_report(source: &Path) -> Result<systemprompt_marketplace::ImportReport, String> {
    let dest = TempDir::new().expect("tempdir");
    import_anthropic_tree(
        source,
        dest.path(),
        &ImportOptions::new(std::env::temp_dir()),
    )
    .map_err(|e| e.to_string())
}

#[test]
fn cowork_is_an_accepted_rule_host() {
    let source = tree("hosts: [cowork, opencode]\n");
    let dest = import(source.path()).expect("cowork is a bundle target");

    assert_eq!(rule_config(dest.path()).hosts, vec!["cowork", "opencode"]);
}

#[test]
fn a_declared_theme_shipped_at_its_pinned_digest_imports() {
    let source = tree("hosts: [opencode]\n");
    with_opencode(source.path(), &sha256_hex(br#"{"theme":{}}"#));

    import(source.path()).expect("the theme digest matches");
}

#[test]
fn a_declared_theme_no_plugin_ships_at_its_digest_is_refused() {
    let source = tree("hosts: [opencode]\n");
    with_opencode(source.path(), THEME_SHA);

    let err = import(source.path()).expect_err("digest mismatch refused");
    assert!(err.contains("opencode theme(s): acme"), "{err}");
}

#[test]
fn a_rule_that_misses_opencode_warns_when_the_marketplace_configures_it() {
    let source = tree("hosts: [claude-code]\n");
    with_opencode(source.path(), &sha256_hex(br#"{"theme":{}}"#));

    let report = import_report(source.path()).expect("a warning, not an error");
    assert!(
        report.warnings.iter().any(|w| matches!(
            w,
            systemprompt_marketplace::ImportWarning::RulesMissOpenCode { rules }
                if rules == &["house_style".to_owned()]
        )),
        "{:?}",
        report.warnings
    );
}

#[test]
fn dev_files_under_the_hosts_tree_are_not_imported() {
    let source = tree("name: House style\n");
    write(
        source.path(),
        "plugins/alpha/hosts/opencode/tests/fixture.json",
        "{}",
    );
    let dest = import(source.path()).expect("import succeeds");

    assert!(
        !dest
            .path()
            .join("plugins/alpha/hosts/opencode/tests")
            .exists()
    );
}
