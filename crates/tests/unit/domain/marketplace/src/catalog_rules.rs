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
    assert_eq!(rules[0].name.as_str(), "Security baseline");
    assert_eq!(rules[0].instructions, "Never paste credentials.");
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
    assert_eq!(rules[0].instructions, "Never paste credentials.");
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

#[test]
fn a_rule_with_tags_and_hosts_carries_them_into_the_entry() {
    let root = TempDir::new().expect("tempdir");
    write_rule(
        root.path(),
        "security",
        "id: security\nname: Security\ndescription: x\ntags: [baseline]\nhosts: [claude-code]\n",
        Some("body\n"),
    );

    let rules = load_rules(root.path()).expect("rules load");
    assert_eq!(rules[0].tags, vec!["baseline"]);
    assert_eq!(rules[0].hosts, vec!["claude-code"]);
    assert!(rules[0].plugins.is_empty());
}

#[test]
fn a_rule_directory_without_a_config_is_skipped_and_traced() {
    let root = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(root.path().join("rules/orphan")).expect("create rule dir");
    std::fs::write(root.path().join("rules/orphan/index.md"), "body\n").expect("write");
    std::fs::write(root.path().join("rules/loose.txt"), "not a rule dir").expect("write");
    write_rule(root.path(), "security", SECURITY_CONFIG, Some("body\n"));

    let _guard = crate::helpers::warn_subscriber_guard();
    let mut trace = systemprompt_marketplace::ManifestTrace::default();
    let rules = systemprompt_marketplace::catalog::load_rules_traced(root.path(), &mut trace)
        .expect("rules load");

    assert_eq!(rules.len(), 1);
    let events = &trace.events;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, "orphan");
    assert_eq!(
        events[0].stage,
        systemprompt_marketplace::TraceStage::DiskScan
    );
    assert!(
        events[0].reason.contains("no config.yaml"),
        "{}",
        events[0].reason
    );
}

#[test]
fn a_disabled_rule_is_traced_as_disabled() {
    let root = TempDir::new().expect("tempdir");
    write_rule(
        root.path(),
        "security",
        "id: security\nname: Security\ndescription: x\nenabled: false\n",
        Some("body\n"),
    );

    let mut trace = systemprompt_marketplace::ManifestTrace::default();
    let rules = systemprompt_marketplace::catalog::load_rules_traced(root.path(), &mut trace)
        .expect("rules load");

    assert!(rules.is_empty());
    let events = &trace.events;
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].stage,
        systemprompt_marketplace::TraceStage::Disabled
    );
}

#[test]
fn a_rule_that_fails_to_build_is_traced_before_the_error_is_returned() {
    let root = TempDir::new().expect("tempdir");
    write_rule(root.path(), "security", SECURITY_CONFIG, None);

    let _guard = crate::helpers::warn_subscriber_guard();
    let mut trace = systemprompt_marketplace::ManifestTrace::default();
    let err = systemprompt_marketplace::catalog::load_rules_traced(root.path(), &mut trace)
        .expect_err("missing content must fail");

    let events = &trace.events;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].stage, systemprompt_marketplace::TraceStage::Parse);
    assert_eq!(events[0].reason, err.to_string());
}

#[test]
fn an_unparsable_rule_config_names_the_file() {
    let root = TempDir::new().expect("tempdir");
    write_rule(root.path(), "security", "id: [unclosed\n", Some("body\n"));

    let _guard = crate::helpers::warn_subscriber_guard();
    let err = load_rules(root.path()).expect_err("bad yaml must fail");

    assert!(err.to_string().contains("config.yaml"), "{err}");
}

#[test]
fn a_rule_with_an_empty_name_is_named_after_its_directory() {
    let root = TempDir::new().expect("tempdir");
    write_rule(
        root.path(),
        "hand_over",
        "id: hand_over\nname: ''\ndescription: x\n",
        Some("body\n"),
    );

    let rules = load_rules(root.path()).expect("rules load");

    assert_eq!(rules[0].name.as_str(), "hand over");
}

#[test]
fn a_rule_names_its_content_file_and_hashes_the_trimmed_body() {
    let root = TempDir::new().expect("tempdir");
    write_rule(
        root.path(),
        "security",
        "id: security\nname: Security\ndescription: x\nfile: RULE.md\n",
        None,
    );
    std::fs::write(root.path().join("rules/security/RULE.md"), "  body  \n\n")
        .expect("write content");

    let rules = load_rules(root.path()).expect("rules load");

    assert!(
        rules[0].file_path.ends_with("RULE.md"),
        "{}",
        rules[0].file_path
    );
    assert_eq!(rules[0].instructions, "body");
    assert_eq!(rules[0].sha256.as_str().len(), 64);
}
