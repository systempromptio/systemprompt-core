use std::collections::BTreeSet;
use std::path::Path;

use systemprompt_identifiers::PluginId;
use systemprompt_marketplace::bundle::BundleContent;
use systemprompt_marketplace::{PluginBundle, build_plugin_bundle};
use systemprompt_models::bridge::ids::{RuleId, RuleName, Sha256Digest};
use systemprompt_models::bridge::manifest::RuleEntry;
use systemprompt_models::services::{
    ComponentSource, PluginAuthor, PluginComponentRef, PluginConfig,
};

const NO_DISABLED: BTreeSet<String> = BTreeSet::new();

fn rule(id: &str, body: &str) -> RuleEntry {
    RuleEntry {
        id: RuleId::try_new(id).expect("rule id"),
        name: RuleName::try_new(id.replace('_', " ")).expect("rule name"),
        description: format!("{id} description"),
        file_path: format!("/nonexistent/rules/{id}/index.md"),
        tags: Vec::new(),
        sha256: Sha256Digest::try_new(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("zero digest"),
        instructions: body.to_owned(),
        hosts: Vec::new(),
        plugins: Vec::new(),
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
        "---\nname: security\ndescription: \"security description\"\n---\n\nNever paste credentials.\n"
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

#[test]
fn a_snake_case_rule_id_is_laid_out_in_kebab_case() {
    let available = vec![rule("data_handling", "Redact first.")];
    let bundle = build(explicit(&["data_handling"]), &available);

    let file = bundle
        .get("rules/data-handling.md")
        .expect("kebab path emitted");
    let text = String::from_utf8(file.bytes.clone()).expect("utf8");
    assert!(text.starts_with("---\nname: data-handling\n"), "{text}");
    assert!(!bundle.contains_key("rules/data_handling.md"));
}

#[test]
fn a_rule_scoped_to_another_host_is_not_emitted() {
    let mut other = rule("security", "a");
    other.hosts = vec!["cursor".to_owned()];
    let bundle = build(explicit(&["security"]), &[other]);

    assert!(!bundle.contains_key("rules/security.md"));
}
