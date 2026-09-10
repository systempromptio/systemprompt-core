use std::path::Path;

use systemprompt_marketplace::catalog::load_rules;
use tempfile::TempDir;

fn write_rule(root: &Path, dir: &str, config: &str, content: Option<&str>) {
    let rule_dir = root.join("rules").join(dir);
    std::fs::create_dir_all(&rule_dir).expect("create rule dir");
    std::fs::write(rule_dir.join("config.yaml"), config).expect("write config");
    if let Some(body) = content {
        std::fs::write(rule_dir.join("index.md"), body).expect("write content");
    }
}

const SECURITY_CONFIG: &str = "id: security\nname: Security baseline\ndescription: Handling \
                               rules.\nenabled: true\nfile: index.md\n";

#[test]
fn a_rule_directory_loads_into_an_entry() {
    let root = TempDir::new().expect("tempdir");
    write_rule(
        root.path(),
        "security",
        SECURITY_CONFIG,
        Some("Never paste credentials.\n"),
    );

    let rules = load_rules(root.path()).expect("rules load");

    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].id.as_str(), "security");
    assert_eq!(rules[0].name, "Security baseline");
    assert_eq!(rules[0].content, "Never paste credentials.");
}

#[test]
fn frontmatter_is_stripped_from_the_rule_body() {
    let root = TempDir::new().expect("tempdir");
    write_rule(
        root.path(),
        "security",
        SECURITY_CONFIG,
        Some("---\nname: Security baseline\n---\n\nNever paste credentials.\n"),
    );

    let rules = load_rules(root.path()).expect("rules load");
    assert_eq!(rules[0].content, "Never paste credentials.");
}

#[test]
fn a_rule_id_that_disagrees_with_its_directory_is_rejected() {
    let root = TempDir::new().expect("tempdir");
    write_rule(
        root.path(),
        "security",
        "id: privacy\nname: Privacy\ndescription: x\n",
        Some("body\n"),
    );

    let err = load_rules(root.path()).expect_err("mismatched id must fail");
    let message = err.to_string();
    assert!(message.contains("privacy"), "{message}");
    assert!(message.contains("security"), "{message}");
}

#[test]
fn a_rule_whose_content_file_is_missing_is_rejected() {
    let root = TempDir::new().expect("tempdir");
    write_rule(root.path(), "security", SECURITY_CONFIG, None);

    let err = load_rules(root.path()).expect_err("missing content must fail");
    assert!(err.to_string().contains("index.md"), "{err}");
}

#[test]
fn a_disabled_rule_is_skipped() {
    let root = TempDir::new().expect("tempdir");
    write_rule(
        root.path(),
        "security",
        "id: security\nname: Security\ndescription: x\nenabled: false\n",
        Some("body\n"),
    );

    assert!(load_rules(root.path()).expect("rules load").is_empty());
}

#[test]
fn a_tree_with_no_rules_directory_loads_nothing() {
    let root = TempDir::new().expect("tempdir");
    assert!(load_rules(root.path()).expect("rules load").is_empty());
}

#[test]
fn rules_load_in_directory_order() {
    let root = TempDir::new().expect("tempdir");
    write_rule(
        root.path(),
        "zeta",
        "id: zeta\nname: Zeta\ndescription: x\n",
        Some("z\n"),
    );
    write_rule(
        root.path(),
        "alpha",
        "id: alpha\nname: Alpha\ndescription: x\n",
        Some("a\n"),
    );

    let rules = load_rules(root.path()).expect("rules load");
    assert_eq!(
        rules.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["alpha", "zeta"]
    );
}
