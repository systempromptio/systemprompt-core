use std::collections::{BTreeMap, BTreeSet};

use systemprompt_identifiers::{MarketplaceId, PluginId};
use systemprompt_models::services::{PluginAuthor, PluginComponentRef, PluginConfig};
use systemprompt_security::authz::types::{Access, AccessRule, Decision, MatchedBy, RuleType};
use systemprompt_security::authz::{
    ChainSources, EntityKind, EntityRef, LoadedParent, MarketplaceSource, ParentChainIndex,
    ResolveBase,
};
use systemprompt_test_fixtures::fixture_user_id;

fn rule(value: &str, access: Access) -> AccessRule {
    AccessRule {
        id: systemprompt_identifiers::RuleId::new(format!("role-{value}-{access}")),
        rule_type: RuleType::ROLE,
        rule_value: value.into(),
        access,
        justification: None,
    }
}

fn plugin_parent(id: &str, rules: Vec<AccessRule>, default_included: Option<bool>) -> LoadedParent {
    LoadedParent {
        entity: EntityRef::Plugin(PluginId::new(id)),
        rules,
        default_included,
    }
}

fn market_parent(rules: Vec<AccessRule>, default_included: Option<bool>) -> LoadedParent {
    LoadedParent {
        entity: EntityRef::Marketplace(MarketplaceId::new("market")),
        rules,
        default_included,
    }
}

fn set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

fn sources(plugins: &[&str], skill_owners: &[(&str, &[&str])]) -> ChainSources {
    ChainSources {
        marketplace: Some(MarketplaceSource {
            id: MarketplaceId::new("market"),
            fallback_default_included: Some(true),
        }),
        plugins: set(plugins),
        skill_owners: skill_owners
            .iter()
            .map(|(skill, owners)| ((*skill).to_owned(), set(owners)))
            .collect(),
        marketplace_members: BTreeMap::from([(EntityKind::McpServer, set(&["odoo"]))]),
    }
}

fn index(plugins: Vec<(&str, LoadedParent)>, sources: ChainSources) -> ParentChainIndex {
    ParentChainIndex::from_parts(
        Some(market_parent(vec![rule("user", Access::Allow)], Some(true))),
        plugins
            .into_iter()
            .map(|(id, parent)| (id.to_owned(), parent))
            .collect(),
        sources,
    )
}

fn decide(index: &ParentChainIndex, kind: EntityKind, id: &str, roles: &[&str]) -> Decision {
    let user = fixture_user_id();
    let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
    index.resolve(
        kind,
        id,
        ResolveBase {
            rules: &[],
            user_id: &user,
            user_roles: &roles,
            default_included: Some(false),
            attributes: &systemprompt_security::authz::NO_SUBJECT_ATTRIBUTES,
            dimensions: &[],
        },
    )
}

#[test]
fn a_skill_chains_through_its_plugin_then_the_marketplace() {
    let index = index(
        vec![("admin-plugin", plugin_parent("admin-plugin", vec![], None))],
        sources(&["admin-plugin"], &[("admin_skill", &["admin-plugin"])]),
    );

    let chains = index.chains_for(EntityKind::Skill, "admin_skill");

    assert_eq!(chains.len(), 1);
    let ids: Vec<&str> = chains[0].iter().map(|p| p.entity.id_str()).collect();
    assert_eq!(ids, ["admin-plugin", "market"]);
}

#[test]
fn a_ruleless_skill_in_an_admin_only_plugin_is_hidden_from_a_user() {
    let index = index(
        vec![(
            "admin-plugin",
            plugin_parent(
                "admin-plugin",
                vec![rule("admin", Access::Allow)],
                Some(false),
            ),
        )],
        sources(&["admin-plugin"], &[("admin_skill", &["admin-plugin"])]),
    );

    assert!(matches!(
        decide(&index, EntityKind::Skill, "admin_skill", &["user"]),
        Decision::Deny { .. }
    ));
    assert!(matches!(
        decide(&index, EntityKind::Skill, "admin_skill", &["user", "admin"]),
        Decision::Allow {
            matched_by: MatchedBy::RoleAllow { .. }
        }
    ));
}

#[test]
fn a_ruleless_skill_in_a_ruleless_plugin_inherits_the_marketplace() {
    let index = index(
        vec![("user-plugin", plugin_parent("user-plugin", vec![], None))],
        sources(&["user-plugin"], &[("crm", &["user-plugin"])]),
    );

    assert!(matches!(
        decide(&index, EntityKind::Skill, "crm", &["user"]),
        Decision::Allow {
            matched_by: MatchedBy::RoleAllow { .. }
        }
    ));
}

#[test]
fn a_skill_owned_by_two_plugins_is_admitted_when_any_owner_admits() {
    let index = index(
        vec![
            (
                "admin-plugin",
                plugin_parent(
                    "admin-plugin",
                    vec![rule("admin", Access::Allow)],
                    Some(false),
                ),
            ),
            ("user-plugin", plugin_parent("user-plugin", vec![], None)),
        ],
        sources(
            &["admin-plugin", "user-plugin"],
            &[("shared", &["admin-plugin", "user-plugin"])],
        ),
    );

    assert_eq!(index.chains_for(EntityKind::Skill, "shared").len(), 2);
    assert!(matches!(
        decide(&index, EntityKind::Skill, "shared", &["user"]),
        Decision::Allow { .. }
    ));
}

#[test]
fn a_plugin_is_parented_by_the_marketplace_and_a_stranger_by_nothing() {
    let index = index(
        vec![("user-plugin", plugin_parent("user-plugin", vec![], None))],
        sources(&["user-plugin"], &[]),
    );

    assert_eq!(index.chains_for(EntityKind::Plugin, "user-plugin").len(), 1);
    assert!(index.chains_for(EntityKind::Plugin, "stranger").is_empty());
    assert_eq!(index.chains_for(EntityKind::McpServer, "odoo").len(), 1);
    assert!(index.chains_for(EntityKind::Hook, "governance").is_empty());
}

#[test]
fn an_entity_with_no_chain_resolves_in_isolation() {
    let index = ParentChainIndex::default();

    assert!(matches!(
        decide(&index, EntityKind::Skill, "orphan", &["user"]),
        Decision::Deny { .. }
    ));
}

fn plugin_config(id: &str, enabled: bool, skills: &[&str]) -> PluginConfig {
    PluginConfig {
        id: PluginId::new(id),
        name: id.to_owned(),
        description: String::new(),
        version: "1.0.0".into(),
        enabled,
        author: PluginAuthor {
            name: "test".into(),
            email: "test@example.com".into(),
        },
        keywords: vec![],
        license: "BSL-1.0".into(),
        category: "test".into(),
        skills: PluginComponentRef {
            include: skills.iter().map(|s| (*s).to_owned()).collect(),
            ..Default::default()
        },
        agents: Default::default(),
        mcp_servers: Default::default(),
        content_sources: Default::default(),
        artifacts: Default::default(),
        hooks: Default::default(),
        scripts: vec![],
    }
}

#[test]
fn from_services_records_which_plugin_selects_each_skill() {
    use systemprompt_models::services::{
        MarketplaceAccess, MarketplaceConfig, MarketplaceVisibility, ServicesConfig,
    };

    let mut services = ServicesConfig::default();
    for plugin in [
        plugin_config("admin-plugin", true, &["admin_skill", "shared"]),
        plugin_config("user-plugin", true, &["crm", "shared"]),
        plugin_config("off-plugin", false, &["never"]),
    ] {
        services
            .plugins
            .insert(plugin.id.as_str().to_owned(), plugin);
    }
    let marketplace = MarketplaceConfig {
        id: MarketplaceId::new("market"),
        name: "market".into(),
        description: String::new(),
        version: "1.0.0".into(),
        enabled: true,
        author: PluginAuthor {
            name: "test".into(),
            email: "test@example.com".into(),
        },
        keywords: vec![],
        license: "BSL-1.0".into(),
        visibility: MarketplaceVisibility::Public,
        plugins: Default::default(),
        mcp_servers: PluginComponentRef {
            include: vec!["odoo".into()],
            ..Default::default()
        },
        agents: Default::default(),
        artifacts: Default::default(),
        access: MarketplaceAccess {
            default_included: true,
            roles: vec!["user".into()],
            attributes: Default::default(),
            justification: None,
        },
    };
    services
        .marketplaces
        .insert(marketplace.id.clone(), marketplace);

    let sources = ChainSources::from_services(&services);

    assert_eq!(sources.plugins, set(&["admin-plugin", "user-plugin"]));
    assert_eq!(sources.skill_owners["admin_skill"], set(&["admin-plugin"]));
    assert_eq!(
        sources.skill_owners["shared"],
        set(&["admin-plugin", "user-plugin"])
    );
    assert!(!sources.skill_owners.contains_key("never"));
    assert!(sources.is_marketplace_member(EntityKind::McpServer, "odoo"));
    assert_eq!(
        sources.marketplace.as_ref().map(|m| m.id.as_str()),
        Some("market")
    );
}
