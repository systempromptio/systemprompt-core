//! Marketplace membership attribute floor and parent cascade for authz
//! enforcement sites.
//!
//! [`member_attribute_floor`] resolves the active marketplace from a
//! [`ServicesConfig`] and, for entities included in it, exposes the
//! declarative `access.attributes` bag. Core never interprets the bag — it is
//! forwarded verbatim to the ABAC hook (via
//! [`super::types::AuthzContext::with_marketplace_floor`]) as a
//! defence-in-depth floor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_models::services::{MarketplaceConfig, MarketplaceMemberKind, ServicesConfig};

use super::types::EntityKind;

#[must_use]
pub fn member_attribute_floor<'a>(
    services: &'a ServicesConfig,
    kind: EntityKind,
    id: &str,
) -> Option<&'a BTreeMap<String, serde_json::Value>> {
    let config = active_marketplace(services)?;
    if config.access.attributes.is_empty() {
        return None;
    }
    is_member(services, config, kind, id).then_some(&config.access.attributes)
}

pub(super) fn active_marketplace(services: &ServicesConfig) -> Option<&MarketplaceConfig> {
    let mut enabled = services.marketplaces.values().filter(|m| m.enabled);
    let first = enabled.next()?;
    if enabled.next().is_none() {
        return Some(first);
    }
    let id = services.settings.default_marketplace_id.as_ref()?;
    services.marketplaces.get(id).filter(|m| m.enabled)
}

enum MemberScope {
    Declared(MarketplaceMemberKind),
    DerivedSkills,
    Unsupported,
}

const fn member_scope(kind: EntityKind) -> MemberScope {
    match kind {
        EntityKind::Plugin => MemberScope::Declared(MarketplaceMemberKind::Plugins),
        EntityKind::Agent => MemberScope::Declared(MarketplaceMemberKind::Agents),
        EntityKind::McpServer => MemberScope::Declared(MarketplaceMemberKind::McpServers),
        EntityKind::Skill => MemberScope::DerivedSkills,
        EntityKind::GatewayRoute
        | EntityKind::Marketplace
        | EntityKind::Hook
        | EntityKind::SlackWorkspace
        | EntityKind::SlackChannel
        | EntityKind::TeamsTenant
        | EntityKind::TeamsConversation => MemberScope::Unsupported,
    }
}

fn is_member(
    services: &ServicesConfig,
    config: &MarketplaceConfig,
    kind: EntityKind,
    id: &str,
) -> bool {
    match member_scope(kind) {
        MemberScope::Declared(member_kind) => config
            .members(member_kind)
            .include
            .iter()
            .any(|member| member == id),
        MemberScope::DerivedSkills => services.marketplace_skill_members(config).contains(id),
        MemberScope::Unsupported => false,
    }
}
