use systemprompt_identifiers::MarketplaceId;
use systemprompt_marketplace::{MarketplaceError, MarketplaceService};
use systemprompt_security::authz::EntityKind;

use crate::helpers::{access, config_with, include, marketplace};

#[test]
fn resolve_default_uses_explicit_id() {
    let mut config = config_with(vec![marketplace("primary"), marketplace("secondary")]);
    config.settings.default_marketplace_id = Some("secondary".into());
    let service = MarketplaceService::new(&config);

    let (id, mp) = service.resolve_default().expect("explicit default resolves");
    assert_eq!(id.as_str(), "secondary");
    assert_eq!(mp.id.as_str(), "secondary");
}

#[test]
fn resolve_default_falls_back_to_conventional_id() {
    let config = config_with(vec![marketplace("default"), marketplace("other")]);
    let service = MarketplaceService::new(&config);

    let (id, _) = service
        .resolve_default()
        .expect("conventional 'default' id resolves");
    assert_eq!(id.as_str(), "default");
}

#[test]
fn resolve_default_errors_when_none() {
    let config = config_with(vec![marketplace("alpha"), marketplace("beta")]);
    let service = MarketplaceService::new(&config);

    assert!(matches!(
        service.resolve_default(),
        Err(MarketplaceError::NoDefault)
    ));
}

#[test]
fn get_hit_returns_config() {
    let config = config_with(vec![marketplace("alpha")]);
    let service = MarketplaceService::new(&config);

    let mp = service
        .get(&MarketplaceId::new("alpha"))
        .expect("existing marketplace is found");
    assert_eq!(mp.id.as_str(), "alpha");
}

#[test]
fn get_miss_returns_not_found() {
    let config = config_with(vec![marketplace("alpha")]);
    let service = MarketplaceService::new(&config);

    assert!(matches!(
        service.get(&MarketplaceId::new("missing")),
        Err(MarketplaceError::NotFound(_))
    ));
}

#[test]
fn membership_maps_members_to_owning_id() {
    let mut mp = marketplace("market");
    mp.skills = include(&["skill-a"]);
    mp.agents = include(&["agent-a"]);
    mp.mcp_servers = include(&["mcp-a"]);
    mp.plugins = include(&["plugin-a"]);
    let config = config_with(vec![mp]);
    let service = MarketplaceService::new(&config);

    let members = service.membership();
    assert_eq!(members.len(), 4);
    assert_eq!(
        members
            .get(&(EntityKind::Skill, "skill-a".to_owned()))
            .map(MarketplaceId::as_str),
        Some("market")
    );
    assert_eq!(
        members
            .get(&(EntityKind::Agent, "agent-a".to_owned()))
            .map(MarketplaceId::as_str),
        Some("market")
    );
    assert_eq!(
        members
            .get(&(EntityKind::McpServer, "mcp-a".to_owned()))
            .map(MarketplaceId::as_str),
        Some("market")
    );
    assert_eq!(
        members
            .get(&(EntityKind::Plugin, "plugin-a".to_owned()))
            .map(MarketplaceId::as_str),
        Some("market")
    );
}

#[test]
fn membership_empty_without_active_marketplace() {
    let config = config_with(vec![]);
    let service = MarketplaceService::new(&config);
    assert!(service.membership().is_empty());
}

#[test]
fn active_access_returns_block_of_active() {
    let mut mp = marketplace("market");
    mp.access = access(true, &["eng"]);
    let config = config_with(vec![mp]);
    let service = MarketplaceService::new(&config);

    let block = service.active_access().expect("active marketplace has access");
    assert!(block.default_included);
    assert_eq!(block.roles, vec!["eng".to_owned()]);
}

#[test]
fn active_access_none_without_active() {
    let config = config_with(vec![]);
    let service = MarketplaceService::new(&config);
    assert!(service.active_access().is_none());
}
