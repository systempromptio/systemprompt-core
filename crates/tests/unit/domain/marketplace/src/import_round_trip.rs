use std::path::Path;

use systemprompt_identifiers::{MarketplaceId, PluginId};
use systemprompt_loader::ConfigLoader;
use systemprompt_marketplace::catalog::load_rules;
use systemprompt_marketplace::{
    BundleContent, ImportOptions, build_plugin_bundle, import_anthropic_tree,
};
use systemprompt_models::bridge::ids::{RuleId, RuleName, Sha256Digest, SkillId, SkillName};
use systemprompt_models::bridge::manifest::{RuleEntry, SkillEntry};
use systemprompt_models::services::marketplace::{
    MarketplaceAccess, MarketplaceAccessRule, MarketplaceConfig, MarketplaceVisibility,
};
use systemprompt_models::services::plugin::{
    ComponentSource, PluginAuthor, PluginComponentRef, PluginConfig,
};
use tempfile::TempDir;

use crate::import_tree::fixture;

fn author() -> PluginAuthor {
    PluginAuthor {
        name: "Acme Field Team".to_owned(),
        email: "field@acme.example".to_owned(),
    }
}

fn explicit(include: &[&str]) -> PluginComponentRef {
    PluginComponentRef {
        source: ComponentSource::Explicit,
        filter: None,
        include: include.iter().map(|s| (*s).to_owned()).collect(),
        exclude: Vec::new(),
    }
}

fn original_plugin() -> PluginConfig {
    PluginConfig {
        id: PluginId::new("alpha-tools"),
        name: "alpha-tools".to_owned(),
        description: "Discovery and reporting for field engagements.".to_owned(),
        version: "1.4.0".to_owned(),
        enabled: true,
        author: author(),
        keywords: vec!["discovery".to_owned(), "field".to_owned()],
        license: "proprietary".to_owned(),
        category: "business".to_owned(),
        skills: explicit(&["alpha_discovery", "alpha_report"]),
        rules: explicit(&["handover"]),
        agents: PluginComponentRef::default(),
        mcp_servers: explicit(&["knowledge-bank"]),
        content_sources: PluginComponentRef::default(),
        artifacts: PluginComponentRef::default(),
        hooks: systemprompt_models::services::plugin::PluginHooksRef::default(),
        scripts: Vec::new(),
    }
}

fn original_marketplace() -> MarketplaceConfig {
    MarketplaceConfig {
        id: MarketplaceId::new("acme-field"),
        name: "acme-field".to_owned(),
        description: "Field engineering tooling for the Acme delivery group.".to_owned(),
        version: "2.1.0".to_owned(),
        enabled: true,
        author: author(),
        keywords: Vec::new(),
        license: "proprietary".to_owned(),
        visibility: MarketplaceVisibility::Private,
        plugins: explicit(&["alpha-tools"]),
        mcp_servers: explicit(&["knowledge-bank"]),
        agents: PluginComponentRef::default(),
        artifacts: PluginComponentRef::default(),
        access: MarketplaceAccess {
            default_included: false,
            roles: Vec::new(),
            rules: vec![MarketplaceAccessRule {
                rule_type: "group".to_owned(),
                values: vec!["field".to_owned()],
                access: systemprompt_models::services::marketplace::MarketplaceRuleAccess::Allow,
                justification: Some("Field delivery group tooling".to_owned()),
            }],
            attributes: Default::default(),
            justification: None,
        },
    }
}

fn skill_entry(id: &str, description: &str) -> SkillEntry {
    SkillEntry {
        id: SkillId::try_new(id).expect("valid skill id"),
        name: SkillName::try_new(id.replace('_', " ")).expect("valid skill name"),
        description: description.to_owned(),
        file_path: format!("/nonexistent/{id}/SKILL.md"),
        tags: vec!["field".to_owned()],
        sha256: Sha256Digest::try_new(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("zero digest"),
        instructions: format!("Instructions for {id}."),
        hosts: Vec::new(),
        plugins: Vec::new(),
    }
}

fn rule_entry(id: &str, body: &str) -> RuleEntry {
    RuleEntry {
        id: RuleId::try_new(id).expect("rule id"),
        name: RuleName::try_new(id).expect("rule name"),
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

fn write_anthropic_tree(root: &Path, plugin: &PluginConfig, marketplace: &MarketplaceConfig) {
    let skills = vec![
        skill_entry("alpha_discovery", "Walk a new field engagement."),
        skill_entry("alpha_report", "Turn engagement notes into a report."),
    ];
    let rules = vec![rule_entry(
        "handover",
        "A handover names the account owner.",
    )];
    let disabled = Default::default();
    let content = BundleContent {
        skills: &skills,
        agents: &[],
        mcp_servers: &[],
        disabled_mcp_servers: &disabled,
        artifacts: &[],
        rules: &rules,
        plugins_root: root,
    };
    let bundle = build_plugin_bundle(plugin, &content).expect("bundle builds");

    let plugin_root = root.join("storage/files/plugins").join(plugin.id.as_str());
    for (rel, file) in &bundle {
        let dest = plugin_root.join(rel);
        std::fs::create_dir_all(dest.parent().expect("parent")).expect("mkdir");
        std::fs::write(&dest, &file.bytes).expect("write bundle file");
    }

    let entries: Vec<serde_json::Value> = marketplace
        .plugins
        .include
        .iter()
        .map(|id| {
            serde_json::json!({
                "name": id,
                "source": format!("./storage/files/plugins/{id}"),
                "description": plugin.description.clone(),
                "version": plugin.version.clone(),
            })
        })
        .collect();
    let manifest = serde_json::json!({
        "name": marketplace.id.as_str(),
        "owner": { "name": marketplace.author.name.clone(), "email": marketplace.author.email.clone() },
        "metadata": {
            "description": marketplace.description.clone(),
            "version": marketplace.version.clone(),
        },
        "plugins": entries,
    });
    std::fs::create_dir_all(root.join(".claude-plugin")).expect("mkdir");
    std::fs::write(
        root.join(".claude-plugin/marketplace.json"),
        serde_json::to_vec_pretty(&manifest).expect("serialise"),
    )
    .expect("write marketplace.json");

    write_sidecars(root, plugin, marketplace);

    std::fs::create_dir_all(root.join("systemprompt/mcp")).expect("mkdir");
    std::fs::copy(
        fixture("anthropic").join("systemprompt/mcp/knowledge-bank.yaml"),
        root.join("systemprompt/mcp/knowledge-bank.yaml"),
    )
    .expect("copy base mcp");
}

fn write_sidecars(root: &Path, plugin: &PluginConfig, marketplace: &MarketplaceConfig) {
    let marketplace_sidecar = serde_yaml::to_string(&serde_json::json!({
        "schema": 1,
        "marketplace": {
            "visibility": "private",
            "enabled": marketplace.enabled,
            "access": marketplace.access,
            "mcp_servers": marketplace.mcp_servers,
        },
    }))
    .expect("serialise marketplace sidecar");
    std::fs::write(
        root.join(".claude-plugin/systemprompt.yaml"),
        marketplace_sidecar,
    )
    .expect("write marketplace sidecar");

    let plugin_sidecar = serde_yaml::to_string(&serde_json::json!({
        "schema": 1,
        "plugin": {
            "category": plugin.category,
            "enabled": plugin.enabled,
            "mcp_servers": plugin.mcp_servers,
        },
    }))
    .expect("serialise plugin sidecar");
    let dir = root
        .join("storage/files/plugins")
        .join(plugin.id.as_str())
        .join(".claude-plugin");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("systemprompt.yaml"), plugin_sidecar).expect("write plugin sidecar");
}

#[test]
fn a_generated_tree_imports_back_to_the_configuration_it_came_from() {
    let plugin = original_plugin();
    let marketplace = original_marketplace();

    let source = TempDir::new().expect("tempdir");
    write_anthropic_tree(source.path(), &plugin, &marketplace);

    let dest = TempDir::new().expect("tempdir");
    import_anthropic_tree(source.path(), dest.path(), &ImportOptions::default())
        .expect("import succeeds");

    let services = ConfigLoader::load_from_path(&dest.path().join("config/config.yaml"))
        .expect("imported tree loads");

    let imported_marketplace = services
        .marketplaces
        .get(&marketplace.id)
        .expect("marketplace round-trips");
    assert_eq!(imported_marketplace.id, marketplace.id);
    assert_eq!(imported_marketplace.description, marketplace.description);
    assert_eq!(imported_marketplace.version, marketplace.version);
    assert_eq!(imported_marketplace.license, marketplace.license);
    assert_eq!(imported_marketplace.visibility, marketplace.visibility);
    assert_eq!(imported_marketplace.enabled, marketplace.enabled);
    assert_eq!(imported_marketplace.author.name, marketplace.author.name);
    assert_eq!(imported_marketplace.author.email, marketplace.author.email);
    assert_eq!(
        imported_marketplace.plugins.include,
        marketplace.plugins.include
    );
    assert_eq!(
        imported_marketplace.mcp_servers.include,
        marketplace.mcp_servers.include
    );
    assert_eq!(imported_marketplace.access.rules.len(), 1);
    assert_eq!(
        imported_marketplace.access.rules[0].values,
        marketplace.access.rules[0].values
    );
    assert_eq!(
        imported_marketplace.access.default_included,
        marketplace.access.default_included
    );

    let imported_plugin = services
        .plugins
        .get(plugin.id.as_str())
        .expect("plugin round-trips");
    assert_eq!(imported_plugin.id, plugin.id);
    assert_eq!(imported_plugin.description, plugin.description);
    assert!(
        imported_plugin
            .version
            .starts_with(&format!("{}+", plugin.version)),
        "the bundle stamps a content hash as semver build metadata: {}",
        imported_plugin.version
    );
    assert_eq!(imported_plugin.category, plugin.category);
    assert_eq!(imported_plugin.license, plugin.license);
    assert_eq!(imported_plugin.enabled, plugin.enabled);
    assert_eq!(imported_plugin.author.name, plugin.author.name);
    assert_eq!(imported_plugin.keywords, plugin.keywords);
    assert_eq!(
        imported_plugin.mcp_servers.include,
        plugin.mcp_servers.include
    );

    assert_eq!(imported_plugin.skills.include, plugin.skills.include);
    assert_eq!(imported_plugin.rules.include, plugin.rules.include);

    let round_tripped_rules = load_rules(dest.path()).expect("rules load");
    assert_eq!(
        round_tripped_rules
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>(),
        vec!["handover"]
    );
    assert_eq!(
        round_tripped_rules[0].instructions,
        "A handover names the account owner."
    );
    assert_eq!(services.skills.skills.len(), plugin.skills.include.len());
}
