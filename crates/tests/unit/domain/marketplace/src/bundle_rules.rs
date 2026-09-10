use std::collections::BTreeSet;
use std::path::Path;

use systemprompt_identifiers::{PluginId, PluginRuleId};
use systemprompt_marketplace::bundle::BundleContent;
use systemprompt_marketplace::catalog::RuleEntry;
use systemprompt_marketplace::{PluginBundle, build_plugin_bundle};
use systemprompt_models::bridge::ids::Sha256Digest;
use systemprompt_models::services::{
    ComponentSource, PluginAuthor, PluginComponentRef, PluginConfig,
};

const NO_DISABLED: BTreeSet<String> = BTreeSet::new();

fn rule(id: &str, body: &str) -> RuleEntry {
    RuleEntry {
        id: PluginRuleId::new(id),
        name: id.replace('-', " "),
        description: format!("{id} description"),
        file_path: format!("/nonexistent/rules/{id}/index.md"),
        sha256: Sha256Digest::try_new(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("zero digest"),
        content: body.to_owned(),
    }
}

fn plugin_with_rules(rules: PluginComponentRef) -> PluginConfig {
    PluginConfig {
        id: PluginId::new("demo-plugin"),
        name: "demo-plugin".to_owned(),
        description: "demo".to_owned(),
        version: "1.0.0".to_owned(),
        enabled: true,
        author: PluginAuthor {
            name: "test".to_owned(),
            email: "test@example.com".to_owned(),
        },
        keywords: vec![],
        license: "BSL-1.0".to_owned(),
        category: "demo".to_owned(),
        skills: PluginComponentRef::default(),
        agents: PluginComponentRef::default(),
        rules,
        mcp_servers: PluginComponentRef::default(),
        content_sources: PluginComponentRef::default(),
        artifacts: PluginComponentRef::default(),
        hooks: Default::default(),
        scripts: vec![],
    }
}

fn build(rules: PluginComponentRef, available: &[RuleEntry]) -> PluginBundle {
    let content = BundleContent {
        skills: &[],
        agents: &[],
        mcp_servers: &[],
        disabled_mcp_servers: &NO_DISABLED,
        artifacts: &[],
        rules: available,
        plugins_root: Path::new("/nonexistent/plugins"),
    };
    build_plugin_bundle(&plugin_with_rules(rules), &content).expect("bundle builds")
}

fn explicit(ids: &[&str]) -> PluginComponentRef {
    PluginComponentRef {
        source: ComponentSource::Explicit,
        include: ids.iter().map(|s| (*s).to_owned()).collect(),
        ..Default::default()
    }
}

#[test]
fn an_included_rule_becomes_a_markdown_file_in_the_bundle() {
    let available = vec![rule("security", "Never paste credentials.")];
    let bundle = build(explicit(&["security"]), &available);

    let file = bundle.get("rules/security.md").expect("rule file emitted");
    assert_eq!(
        String::from_utf8(file.bytes.clone()).expect("utf8"),
        "Never paste credentials.\n"
    );
    assert!(!file.executable);
}

#[test]
fn a_rule_the_plugin_does_not_include_is_not_emitted() {
    let available = vec![rule("security", "a"), rule("privacy", "b")];
    let bundle = build(explicit(&["security"]), &available);

    assert!(bundle.contains_key("rules/security.md"));
    assert!(!bundle.contains_key("rules/privacy.md"));
}

#[test]
fn an_instance_sourced_plugin_takes_every_rule_but_its_exclusions() {
    let available = vec![rule("security", "a"), rule("privacy", "b")];
    let selection = PluginComponentRef {
        source: ComponentSource::Instance,
        exclude: vec!["privacy".to_owned()],
        ..Default::default()
    };
    let bundle = build(selection, &available);

    assert!(bundle.contains_key("rules/security.md"));
    assert!(!bundle.contains_key("rules/privacy.md"));
}

#[test]
fn a_plugin_with_no_rules_emits_no_rules_directory() {
    let available = vec![rule("security", "a")];
    let bundle = build(PluginComponentRef::default(), &available);

    assert!(!bundle.keys().any(|k| k.starts_with("rules/")));
}

#[test]
fn including_a_rule_changes_the_bundle_content_version() {
    let available = vec![rule("security", "a")];
    let without = build(PluginComponentRef::default(), &available);
    let with = build(explicit(&["security"]), &available);

    let version = |b: &PluginBundle| -> String {
        let bytes = &b
            .get(".claude-plugin/plugin.json")
            .expect("manifest")
            .bytes
            .clone();
        let json: serde_json::Value = serde_json::from_slice(bytes).expect("valid json");
        json["version"].as_str().expect("version").to_owned()
    };

    assert_ne!(version(&without), version(&with));
}
