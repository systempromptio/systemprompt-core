use std::fs;

use serde_json::{Value, json};
use systemprompt_harness::projection::{AGENTS_FILE, BundleFiles, RULES_BEGIN, RULES_END};
use systemprompt_models::bridge::host::HostKind;

use crate::projection::fixtures;

fn rule(id: &str, name: &str, instructions: &str) -> Value {
    json!({ "id": id, "name": name, "instructions": instructions, "hosts": [HostKind::Harness.as_str()] })
}

fn block_of(rules: &[Value]) -> String {
    let manifest = fixtures::manifest(json!({ "rules": rules }));
    fixtures::projected(&manifest, &BundleFiles::new())
        .rules_block
        .unwrap_or_default()
}

fn spliced(existing: Option<&str>, rules: &[Value]) -> Option<String> {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    let agents = applier.config_dir().join(AGENTS_FILE);
    if let Some(text) = existing {
        fs::create_dir_all(applier.config_dir()).unwrap();
        fs::write(&agents, text).unwrap();
    }
    let manifest = fixtures::manifest(json!({ "rules": rules }));
    applier
        .apply(&fixtures::projected(&manifest, &BundleFiles::new()))
        .unwrap();
    fs::read_to_string(&agents).ok()
}

#[test]
fn the_block_matches_the_bridge_byte_for_byte() {
    let text = block_of(&[
        rule("b", "Second", "  second\n"),
        rule("a", "First", "first"),
    ]);
    assert_eq!(
        text,
        "<!-- systemprompt:begin (managed by the bridge; edits are replaced) -->\n\n## First\n\nfirst\n\n## Second\n\nsecond\n\n<!-- systemprompt:end -->\n"
    );
    assert!(text.starts_with(RULES_BEGIN));
    assert!(text.ends_with(&format!("{RULES_END}\n")));
    let none = fixtures::projected(&fixtures::manifest(json!({})), &BundleFiles::new());
    assert_eq!(none.rules_block, None);
}

#[test]
fn rules_need_the_host_and_a_plain_id() {
    let mut other = rule("other", "Other", "x");
    other["hosts"] = json!(["claude-code"]);
    let mut unscoped = rule("unscoped", "Unscoped", "x");
    unscoped["hosts"] = json!([]);
    let odd = rule("has space", "Odd", "x");
    let manifest =
        fixtures::manifest(json!({ "rules": [rule("kept", "Kept", ""), other, unscoped, odd] }));
    let projection = fixtures::projected(&manifest, &BundleFiles::new());
    assert_eq!(projection.counts.rules, 1);
    assert_eq!(projection.warnings.len(), 1);
    let block = projection.rules_block.unwrap_or_default();
    assert!(block.contains("\n## Kept\n\n\n"));
    assert!(!block.contains("Other") && !block.contains("Unscoped") && !block.contains("Odd"));
}

#[test]
fn a_file_of_its_own_holds_only_the_block() {
    let a = block_of(&[rule("a", "First", "first")]);
    assert_eq!(
        spliced(None, &[rule("a", "First", "first")]),
        Some(a.clone())
    );
    assert_eq!(spliced(Some(&a), &[]), None);
    assert_eq!(spliced(None, &[]), None);
}

#[test]
fn the_users_text_stays_and_the_block_goes_last() {
    let a_rules = || vec![rule("a", "First", "first")];
    let b_rules = || vec![rule("b", "Second", "second")];
    let a = block_of(&a_rules());
    let b = block_of(&b_rules());
    let mine = "# Mine\n\nKeep this.\n";
    let appended = spliced(Some(mine), &a_rules()).unwrap_or_default();
    assert_eq!(appended, format!("{mine}\n{a}"));
    assert_eq!(
        spliced(Some(&appended), &b_rules()),
        Some(format!("{mine}\n{b}"))
    );
    assert_eq!(spliced(Some(&appended), &[]), Some(mine.to_owned()));
    assert_eq!(
        spliced(Some(&format!("# Top\n\n{a}\n# Bottom\n")), &b_rules()),
        Some(format!("# Top# Bottom\n\n{b}"))
    );
    assert_eq!(spliced(Some("# Mine\n"), &[]), Some("# Mine\n".to_owned()));
    let old = block_of(&[rule("old", "Old", "stale")]);
    assert_eq!(
        spliced(Some(&format!("# Mine\n\n{old}")), &a_rules()),
        Some(format!("# Mine\n\n{a}"))
    );
}
