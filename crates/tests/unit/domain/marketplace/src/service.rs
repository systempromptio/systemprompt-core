use systemprompt_identifiers::MarketplaceId;
use systemprompt_marketplace::{MarketplaceError, MarketplaceService};
use systemprompt_security::authz::EntityKind;

use crate::helpers::{access, config_with, include, marketplace};

#[test]
fn resolve_default_uses_explicit_id() {
    let mut config = config_with(vec![marketplace("primary"), marketplace("secondary")]);
    config.settings.default_marketplace_id = Some(MarketplaceId::new("secondary"));
    let service = MarketplaceService::new(&config);

    let (id, mp) = service
        .resolve_default()
        .expect("explicit default resolves");
    assert_eq!(id.as_str(), "secondary");
    assert_eq!(mp.id.as_str(), "secondary");
}

#[test]
fn resolve_default_uses_sole_marketplace_without_explicit_id() {
    let config = config_with(vec![marketplace("only")]);
    let service = MarketplaceService::new(&config);

    let (id, mp) = service
        .resolve_default()
        .expect("the single configured marketplace resolves");
    assert_eq!(id.as_str(), "only");
    assert_eq!(mp.id.as_str(), "only");
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

    let block = service
        .active_access()
        .expect("active marketplace has access");
    assert!(block.default_included);
    assert_eq!(block.roles, vec!["eng".to_owned()]);
}

#[test]
fn active_access_none_without_active() {
    let config = config_with(vec![]);
    let service = MarketplaceService::new(&config);
    assert!(service.active_access().is_none());
}

#[test]
fn active_none_when_ambiguous_without_default() {
    let config = config_with(vec![marketplace("alpha"), marketplace("beta")]);
    let service = MarketplaceService::new(&config);
    assert!(service.active().is_none());
}

#[test]
fn active_selects_default_when_many() {
    let mut config = config_with(vec![marketplace("alpha"), marketplace("beta")]);
    config.settings.default_marketplace_id = Some(MarketplaceId::new("beta"));
    let service = MarketplaceService::new(&config);

    let active = service
        .active()
        .expect("default names the active marketplace");
    assert_eq!(active.id.as_str(), "beta");
}

#[test]
fn member_attribute_floor_returns_block_for_member() {
    let mut mp = marketplace("market");
    mp.mcp_servers = include(&["sharepoint-sim"]);
    mp.access.attributes.insert(
        "boeing.clearance".to_owned(),
        serde_json::json!(["Internal", "CUI"]),
    );
    let config = config_with(vec![mp]);
    let service = MarketplaceService::new(&config);

    let floor = service
        .member_attribute_floor(EntityKind::McpServer, "sharepoint-sim")
        .expect("member inherits the marketplace floor");
    assert_eq!(
        floor.get("boeing.clearance"),
        Some(&serde_json::json!(["Internal", "CUI"]))
    );
}

#[test]
fn member_attribute_floor_none_for_non_member() {
    let mut mp = marketplace("market");
    mp.mcp_servers = include(&["sharepoint-sim"]);
    mp.access.attributes.insert(
        "boeing.clearance".to_owned(),
        serde_json::json!(["Internal"]),
    );
    let config = config_with(vec![mp]);
    let service = MarketplaceService::new(&config);

    assert!(
        service
            .member_attribute_floor(EntityKind::McpServer, "other-server")
            .is_none()
    );
}

#[test]
fn validate_referential_integrity_passes_for_consistent_config() {
    let config = config_with(vec![marketplace("solo")]);
    let service = MarketplaceService::new(&config);
    service
        .validate_referential_integrity()
        .expect("a self-consistent services config validates");
}

#[test]
fn validate_referential_integrity_flags_dangling_reference() {
    let mut mp = marketplace("market");
    mp.plugins = include(&["never-defined-plugin"]);
    let config = config_with(vec![mp]);
    let service = MarketplaceService::new(&config);

    assert!(
        matches!(
            service.validate_referential_integrity(),
            Err(MarketplaceError::Validation(_))
        ),
        "a marketplace referencing an undefined plugin fails referential-integrity validation",
    );
}

#[test]
fn member_attribute_floor_none_when_attributes_empty() {
    let mut mp = marketplace("market");
    mp.mcp_servers = include(&["sharepoint-sim"]);
    let config = config_with(vec![mp]);
    let service = MarketplaceService::new(&config);

    assert!(
        service
            .member_attribute_floor(EntityKind::McpServer, "sharepoint-sim")
            .is_none()
    );
}
