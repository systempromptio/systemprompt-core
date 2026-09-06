use std::collections::{BTreeMap, BTreeSet};

use systemprompt_identifiers::{MarketplaceId, PluginId, SkillId};
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

fn market_parent_named(
    id: &str,
    rules: Vec<AccessRule>,
    default_included: Option<bool>,
) -> LoadedParent {
    LoadedParent {
        entity: EntityRef::Marketplace(MarketplaceId::new(id)),
        rules,
        default_included,
    }
}

fn market_set(values: &[&str]) -> BTreeSet<MarketplaceId> {
    values.iter().map(|v| MarketplaceId::new(*v)).collect()
}

fn market_source(id: &str) -> (MarketplaceId, MarketplaceSource) {
    (
        MarketplaceId::new(id),
        MarketplaceSource {
            id: MarketplaceId::new(id),
            fallback_default_included: Some(true),
        },
    )
}

fn plugin_set(values: &[&str]) -> BTreeSet<PluginId> {
    values.iter().map(|v| PluginId::new(*v)).collect()
}

fn sources(plugins: &[&str], skill_owners: &[(&str, &[&str])]) -> ChainSources {
    sources_in(plugins, skill_owners, &["market"])
}

fn sources_in(
    plugins: &[&str],
    skill_owners: &[(&str, &[&str])],
    markets: &[&str],
) -> ChainSources {
    ChainSources {
        marketplaces: markets.iter().map(|m| market_source(m)).collect(),
        plugins: plugins
            .iter()
            .map(|p| (PluginId::new(*p), market_set(markets)))
            .collect(),
        skill_owners: skill_owners
            .iter()
            .map(|(skill, owners)| (SkillId::new(*skill), plugin_set(owners)))
            .collect(),
        marketplace_members: BTreeMap::from([(
            EntityKind::McpServer,
            BTreeMap::from([("odoo".to_owned(), market_set(markets))]),
        )]),
    }
}

fn index(plugins: Vec<(&str, LoadedParent)>, sources: ChainSources) -> ParentChainIndex {
    index_with(
        vec![(
            "market",
            market_parent_named("market", vec![rule("user", Access::Allow)], Some(true)),
        )],
        plugins,
        sources,
    )
}

fn index_with(
    markets: Vec<(&str, LoadedParent)>,
    plugins: Vec<(&str, LoadedParent)>,
    sources: ChainSources,
) -> ParentChainIndex {
    ParentChainIndex::from_parts(
        markets
            .into_iter()
            .map(|(id, parent)| (MarketplaceId::new(id), parent))
            .collect(),
        plugins
            .into_iter()
            .map(|(id, parent)| (PluginId::new(id), parent))
            .collect(),
        std::sync::Arc::new(sources),
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
            rules: vec![],
            attributes: Default::default(),
            justification: None,
        },
    };
    services
        .marketplaces
        .insert(marketplace.id.clone(), marketplace);

    let sources = ChainSources::from_services(&services);

    assert_eq!(
        sources.plugins.keys().cloned().collect::<BTreeSet<_>>(),
        plugin_set(&["admin-plugin", "user-plugin"])
    );
    assert_eq!(
        sources.skill_owners["admin_skill"],
        plugin_set(&["admin-plugin"])
    );
    assert_eq!(
        sources.skill_owners["shared"],
        plugin_set(&["admin-plugin", "user-plugin"])
    );
    assert!(!sources.skill_owners.contains_key("never"));
    assert!(sources.is_marketplace_member(EntityKind::McpServer, "odoo"));
    assert_eq!(
        sources
            .marketplaces
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        market_set(&["market"])
    );
    assert_eq!(
        sources.plugin_marketplaces(&PluginId::new("admin-plugin")),
        &market_set(&["market"])
    );
}

fn two_market_index(
    plugin_rules: Vec<AccessRule>,
    alpha_rules: Vec<AccessRule>,
    beta_rules: Vec<AccessRule>,
) -> ParentChainIndex {
    index_with(
        vec![
            (
                "alpha",
                market_parent_named("alpha", alpha_rules, Some(false)),
            ),
            ("beta", market_parent_named("beta", beta_rules, Some(false))),
        ],
        vec![(
            "shared-plugin",
            plugin_parent("shared-plugin", plugin_rules, None),
        )],
        sources_in(
            &["shared-plugin"],
            &[("shared_skill", &["shared-plugin"])],
            &["alpha", "beta"],
        ),
    )
}

#[test]
fn plugin_in_two_marketplaces_is_allowed_when_either_admits() {
    let index = two_market_index(
        vec![],
        vec![rule("admin", Access::Allow)],
        vec![rule("user", Access::Allow)],
    );

    assert_eq!(
        index.chains_for(EntityKind::Plugin, "shared-plugin").len(),
        2
    );
    assert!(matches!(
        decide(&index, EntityKind::Plugin, "shared-plugin", &["user"]),
        Decision::Allow { .. }
    ));
    assert!(matches!(
        decide(&index, EntityKind::Plugin, "shared-plugin", &["admin"]),
        Decision::Allow { .. }
    ));
}

#[test]
fn plugin_in_two_marketplaces_denied_by_both_reports_first_deny_in_id_order() {
    let index = two_market_index(
        vec![],
        vec![rule("admin", Access::Allow)],
        vec![rule("admin", Access::Allow)],
    );

    let chains = index.chains_for(EntityKind::Plugin, "shared-plugin");
    let ids: Vec<&str> = chains.iter().map(|c| c[0].entity.id_str()).collect();
    assert_eq!(
        ids,
        ["alpha", "beta"],
        "chains are ordered by marketplace id"
    );
    assert!(matches!(
        decide(&index, EntityKind::Plugin, "shared-plugin", &["user"]),
        Decision::Deny { .. }
    ));
}

#[test]
fn entity_level_deny_beats_every_owning_marketplace() {
    let index = two_market_index(
        vec![],
        vec![rule("user", Access::Allow)],
        vec![rule("user", Access::Allow)],
    );

    let user = fixture_user_id();
    let roles = vec!["user".to_owned()];
    let decision = index.resolve(
        EntityKind::Plugin,
        "shared-plugin",
        ResolveBase {
            rules: &[rule("user", Access::Deny)],
            user_id: &user,
            user_roles: &roles,
            default_included: Some(true),
            attributes: &systemprompt_security::authz::NO_SUBJECT_ATTRIBUTES,
            dimensions: &[],
        },
    );
    assert!(matches!(decision, Decision::Deny { .. }));
}

#[test]
fn plugin_level_rule_closes_cascade_before_any_marketplace() {
    let index = two_market_index(
        vec![rule("user", Access::Deny)],
        vec![rule("user", Access::Allow)],
        vec![rule("user", Access::Allow)],
    );

    assert!(matches!(
        decide(&index, EntityKind::Skill, "shared_skill", &["user"]),
        Decision::Deny { .. }
    ));
}

#[test]
fn skill_owned_by_plugins_in_different_marketplaces_gets_one_chain_per_pair() {
    let mut sources = sources_in(
        &["shared-plugin"],
        &[("shared_skill", &["shared-plugin", "solo-plugin"])],
        &["alpha", "beta"],
    );
    sources
        .plugins
        .insert(PluginId::new("solo-plugin"), market_set(&["beta"]));

    let index = index_with(
        vec![
            ("alpha", market_parent_named("alpha", vec![], Some(true))),
            ("beta", market_parent_named("beta", vec![], Some(true))),
        ],
        vec![
            (
                "shared-plugin",
                plugin_parent("shared-plugin", vec![], None),
            ),
            ("solo-plugin", plugin_parent("solo-plugin", vec![], None)),
        ],
        sources,
    );

    let chains = index.chains_for(EntityKind::Skill, "shared_skill");
    let pairs: Vec<Vec<&str>> = chains
        .iter()
        .map(|c| c.iter().map(|p| p.entity.id_str()).collect())
        .collect();
    assert_eq!(
        pairs,
        vec![
            vec!["shared-plugin", "alpha"],
            vec!["shared-plugin", "beta"],
            vec!["solo-plugin", "beta"],
        ]
    );
}

#[test]
fn orphan_skill_falls_back_to_every_marketplace() {
    let mut sources = sources_in(&["shared-plugin"], &[], &["alpha", "beta"]);
    sources.marketplace_members.insert(
        EntityKind::Skill,
        BTreeMap::from([("orphan".to_owned(), market_set(&["alpha", "beta"]))]),
    );

    let index = index_with(
        vec![
            ("alpha", market_parent_named("alpha", vec![], Some(true))),
            ("beta", market_parent_named("beta", vec![], Some(true))),
        ],
        vec![],
        sources,
    );

    let chains = index.chains_for(EntityKind::Skill, "orphan");
    let ids: Vec<&str> = chains.iter().map(|c| c[0].entity.id_str()).collect();
    assert_eq!(ids, ["alpha", "beta"]);
}
