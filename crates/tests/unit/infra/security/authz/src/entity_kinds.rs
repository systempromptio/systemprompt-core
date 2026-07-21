use std::str::FromStr;

use systemprompt_identifiers::{
    AgentId, HookId, MarketplaceId, McpServerId, PluginId, RouteId, SkillId, SlackChannelId,
    SlackWorkspaceId, TeamsConversationId, TeamsTenantId,
};
use systemprompt_security::authz::{Access, EntityKind, EntityRef, RuleType};

#[test]
fn entity_kind_as_str_all_variants() {
    assert_eq!(EntityKind::GatewayRoute.as_str(), "gateway_route");
    assert_eq!(EntityKind::McpServer.as_str(), "mcp_server");
    assert_eq!(EntityKind::Plugin.as_str(), "plugin");
    assert_eq!(EntityKind::Agent.as_str(), "agent");
    assert_eq!(EntityKind::Marketplace.as_str(), "marketplace");
    assert_eq!(EntityKind::Skill.as_str(), "skill");
    assert_eq!(EntityKind::Hook.as_str(), "hook");
    assert_eq!(EntityKind::SlackWorkspace.as_str(), "slack_workspace");
    assert_eq!(EntityKind::SlackChannel.as_str(), "slack_channel");
    assert_eq!(EntityKind::TeamsTenant.as_str(), "teams_tenant");
    assert_eq!(EntityKind::TeamsConversation.as_str(), "teams_conversation");
}

#[test]
fn entity_kind_from_str_valid() {
    assert_eq!(
        EntityKind::from_str("gateway_route").unwrap(),
        EntityKind::GatewayRoute
    );
    assert_eq!(
        EntityKind::from_str("mcp_server").unwrap(),
        EntityKind::McpServer
    );
    assert_eq!(EntityKind::from_str("plugin").unwrap(), EntityKind::Plugin);
    assert_eq!(EntityKind::from_str("agent").unwrap(), EntityKind::Agent);
    assert_eq!(
        EntityKind::from_str("marketplace").unwrap(),
        EntityKind::Marketplace
    );
    assert_eq!(EntityKind::from_str("skill").unwrap(), EntityKind::Skill);
    assert_eq!(EntityKind::from_str("hook").unwrap(), EntityKind::Hook);
    assert_eq!(
        EntityKind::from_str("slack_workspace").unwrap(),
        EntityKind::SlackWorkspace
    );
    assert_eq!(
        EntityKind::from_str("slack_channel").unwrap(),
        EntityKind::SlackChannel
    );
    assert_eq!(
        EntityKind::from_str("teams_tenant").unwrap(),
        EntityKind::TeamsTenant
    );
    assert_eq!(
        EntityKind::from_str("teams_conversation").unwrap(),
        EntityKind::TeamsConversation
    );
}

#[test]
fn entity_kind_from_str_invalid() {
    assert!(EntityKind::from_str("widget").is_err());
    assert!(EntityKind::from_str("").is_err());
    assert!(EntityKind::from_str("GatewayRoute").is_err());
}

#[test]
fn entity_kind_display() {
    assert_eq!(format!("{}", EntityKind::GatewayRoute), "gateway_route");
    assert_eq!(format!("{}", EntityKind::Plugin), "plugin");
}

#[test]
fn entity_ref_kind_and_id_str_all_variants() {
    let cases: &[(EntityRef, EntityKind, &str)] = &[
        (
            EntityRef::GatewayRoute(RouteId::new("r1")),
            EntityKind::GatewayRoute,
            "r1",
        ),
        (
            EntityRef::McpServer(McpServerId::new("ms1")),
            EntityKind::McpServer,
            "ms1",
        ),
        (
            EntityRef::Plugin(PluginId::new("p1")),
            EntityKind::Plugin,
            "p1",
        ),
        (
            EntityRef::Agent(AgentId::new("a1")),
            EntityKind::Agent,
            "a1",
        ),
        (
            EntityRef::Marketplace(MarketplaceId::new("m1")),
            EntityKind::Marketplace,
            "m1",
        ),
        (
            EntityRef::Skill(SkillId::new("s1")),
            EntityKind::Skill,
            "s1",
        ),
        (EntityRef::Hook(HookId::new("h1")), EntityKind::Hook, "h1"),
        (
            EntityRef::SlackWorkspace(SlackWorkspaceId::new("T123")),
            EntityKind::SlackWorkspace,
            "T123",
        ),
        (
            EntityRef::SlackChannel(SlackChannelId::new("C456")),
            EntityKind::SlackChannel,
            "C456",
        ),
        (
            EntityRef::TeamsTenant(TeamsTenantId::new("tenant-1")),
            EntityKind::TeamsTenant,
            "tenant-1",
        ),
        (
            EntityRef::TeamsConversation(TeamsConversationId::new("conv-1")),
            EntityKind::TeamsConversation,
            "conv-1",
        ),
    ];
    for (entity, expected_kind, expected_id) in cases {
        assert_eq!(entity.kind(), *expected_kind, "wrong kind for {entity:?}");
        assert_eq!(entity.id_str(), *expected_id, "wrong id for {entity:?}");
    }
}

#[test]
fn entity_ref_display_format() {
    let r = EntityRef::GatewayRoute(RouteId::new("my-route"));
    assert_eq!(r.to_string(), "gateway_route:my-route");

    let p = EntityRef::Plugin(PluginId::new("my-plugin"));
    assert_eq!(p.to_string(), "plugin:my-plugin");
}

#[test]
fn entity_ref_serde_roundtrip() {
    let refs = vec![
        EntityRef::GatewayRoute(RouteId::new("r1")),
        EntityRef::McpServer(McpServerId::new("ms1")),
        EntityRef::Plugin(PluginId::new("p1")),
        EntityRef::Agent(AgentId::new("a1")),
        EntityRef::Marketplace(MarketplaceId::new("m1")),
        EntityRef::Skill(SkillId::new("s1")),
        EntityRef::Hook(HookId::new("h1")),
        EntityRef::SlackWorkspace(SlackWorkspaceId::new("T123")),
        EntityRef::SlackChannel(SlackChannelId::new("C456")),
        EntityRef::TeamsTenant(TeamsTenantId::new("tenant-1")),
        EntityRef::TeamsConversation(TeamsConversationId::new("conv-1")),
    ];
    for entity in refs {
        let s = serde_json::to_string(&entity).unwrap();
        let back: EntityRef = serde_json::from_str(&s).unwrap();
        assert_eq!(back.id_str(), entity.id_str());
        assert_eq!(back.kind(), entity.kind());
    }
}

#[test]
fn rule_type_from_str_valid() {
    assert_eq!(RuleType::from_str("user").unwrap(), RuleType::USER);
    assert_eq!(RuleType::from_str("role").unwrap(), RuleType::ROLE);
}

/// `rule_type` is an open vocabulary, so parsing a stored row never fails on
/// an unrecognised slug — an extension dimension core has never heard of must
/// round-trip rather than poison the read. Minting a *new* slug is where the
/// shape rules apply; see `RuleType::extension`.
#[test]
fn rule_type_from_str_accepts_unknown_slugs() {
    let parsed = RuleType::from_str("group").expect("unknown slug is data, not an error");
    assert_eq!(parsed.as_str(), "group");
    assert_ne!(parsed, RuleType::USER);
    assert_ne!(parsed, RuleType::ROLE);
}

#[test]
fn rule_type_display() {
    assert_eq!(format!("{}", RuleType::USER), "user");
    assert_eq!(format!("{}", RuleType::ROLE), "role");
}

#[test]
fn access_from_str_valid() {
    assert_eq!(Access::from_str("allow").unwrap(), Access::Allow);
    assert_eq!(Access::from_str("deny").unwrap(), Access::Deny);
}

#[test]
fn access_from_str_invalid() {
    assert!(Access::from_str("permit").is_err());
    assert!(Access::from_str("").is_err());
}

#[test]
fn access_display() {
    assert_eq!(format!("{}", Access::Allow), "allow");
    assert_eq!(format!("{}", Access::Deny), "deny");
}
