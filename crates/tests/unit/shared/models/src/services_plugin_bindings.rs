use std::collections::HashMap;

use systemprompt_identifiers::{MarketplaceId, PluginId};
use systemprompt_manifest::services::{
    MarketplaceConfig, MarketplaceVisibility, PluginAuthor, PluginConfig, ServicesConfig,
};
use systemprompt_models::plugin::{PluginComponentRef, PluginDependency, PluginHooksRef};

fn author() -> PluginAuthor {
    PluginAuthor {
        name: "Ed".to_owned(),
        email: "ed@example.com".to_owned(),
    }
}

fn plugin(id: &str) -> PluginConfig {
    PluginConfig {
        id: PluginId::new(id),
        name: id.to_owned(),
        description: "d".to_owned(),
        version: "1.0.0".to_owned(),
        enabled: true,
        author: author(),
        keywords: vec![],
        license: "MIT".to_owned(),
        category: "dev".to_owned(),
        skills: PluginComponentRef::default(),
        agents: PluginComponentRef::default(),
        rules: PluginComponentRef::default(),
        mcp_servers: PluginComponentRef::default(),
        content_sources: PluginComponentRef::default(),
        artifacts: PluginComponentRef::default(),
        hooks: Default::default(),
        scripts: vec![],
        dependencies: vec![],
    }
}

fn marketplace(id: &str, plugins: &[&str]) -> MarketplaceConfig {
    MarketplaceConfig {
        id: MarketplaceId::new(id),
        name: id.to_owned(),
        description: String::new(),
        version: "1.0.0".to_owned(),
        enabled: true,
        author: author(),
        keywords: vec![],
        license: "MIT".to_owned(),
        visibility: MarketplaceVisibility::Public,
        plugins: PluginComponentRef {
            include: plugins.iter().map(|p| (*p).to_owned()).collect(),
            ..Default::default()
        },
        mcp_servers: PluginComponentRef::default(),
        agents: PluginComponentRef::default(),
        artifacts: PluginComponentRef::default(),
        access: Default::default(),
        allow_cross_marketplace_dependencies_on: vec![],
        external_marketplaces: vec![],
        opencode: None,
        external_plugins: vec![],
        claude_code: None,
    }
}

fn services(plugins: Vec<PluginConfig>, marketplaces: Vec<MarketplaceConfig>) -> ServicesConfig {
    let mut config = ServicesConfig::default();
    config.plugins = plugins
        .into_iter()
        .map(|p| (p.id.as_str().to_owned(), p))
        .collect::<HashMap<_, _>>();
    config.marketplaces = marketplaces
        .into_iter()
        .map(|m| (m.id.clone(), m))
        .collect();
    config
}

fn hooks(governance: bool, judge: bool) -> PluginHooksRef {
    PluginHooksRef {
        governance,
        judge,
        ..PluginHooksRef::default()
    }
}

#[test]
fn a_disabled_member_plugin_and_dangling_excludes_only_warn() {
    let mut quiet = plugin("quiet");
    quiet.enabled = false;
    let mut excluding = plugin("excluding");
    excluding.skills.exclude = vec!["no-such-skill".to_owned()];
    excluding.agents.exclude = vec!["no-such-agent".to_owned()];
    services(
        vec![quiet, excluding],
        vec![marketplace("org", &["quiet", "excluding"])],
    )
    .validate()
    .expect("warnings do not fail validation");
}

#[test]
fn a_dependency_naming_its_own_marketplace_is_carried_implicitly() {
    let mut app = plugin("app");
    app.dependencies = vec![PluginDependency {
        name: "helper".to_owned(),
        marketplace: Some("org".to_owned()),
        version: None,
    }];
    services(
        vec![app, plugin("helper")],
        vec![marketplace("org", &["app"])],
    )
    .validate()
    .expect("a self-marketplace dependency needs no allowlist");
}

#[test]
fn an_unknown_agent_in_a_marketplace_is_refused() {
    let mut org = marketplace("org", &[]);
    org.agents.include = vec!["ghost-agent".to_owned()];
    let err = services(vec![], vec![org])
        .validate()
        .expect_err("unknown agent");
    assert!(err.to_string().contains("ghost-agent"), "{err}");
}

#[test]
fn an_unknown_mcp_server_in_a_marketplace_is_refused() {
    let mut org = marketplace("org", &[]);
    org.mcp_servers.include = vec!["ghost-mcp".to_owned()];
    let err = services(vec![], vec![org])
        .validate()
        .expect_err("unknown mcp server");
    assert!(err.to_string().contains("ghost-mcp"), "{err}");
}

#[test]
fn a_single_judge_owner_with_governance_validates() {
    let mut owner = plugin("owner");
    owner.hooks = hooks(true, true);
    services(vec![owner], vec![])
        .validate()
        .expect("one judge owner");
}

#[test]
fn two_judge_owners_are_refused() {
    let mut first = plugin("first");
    first.hooks = hooks(true, true);
    let mut second = plugin("second");
    second.hooks = hooks(true, true);
    let err = services(vec![first, second], vec![])
        .validate()
        .expect_err("two judges");
    let message = err.to_string();
    assert!(message.contains("Multiple plugins"), "{message}");
    assert!(
        message.contains("first") && message.contains("second"),
        "{message}"
    );
}

#[test]
fn a_judge_without_governance_is_refused() {
    let mut orphan = plugin("orphan");
    orphan.hooks = hooks(false, true);
    let err = services(vec![orphan], vec![])
        .validate()
        .expect_err("orphan judge");
    assert!(err.to_string().contains("hooks.governance"), "{err}");
}
