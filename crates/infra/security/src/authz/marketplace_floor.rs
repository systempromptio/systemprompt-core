//! Marketplace membership attribute floor and parent cascade for authz
//! enforcement sites.
//!
//! [`member_attribute_floor`] walks every enabled marketplace in a
//! [`ServicesConfig`] and, for entities included in one, merges the declarative
//! `access.attributes` bags into one floor. Core never interprets the bags —
//! they are forwarded verbatim to the ABAC hook (via
//! [`super::types::AuthzContext::with_marketplace_floor`]) as a
//! defence-in-depth floor.
//!
//! Merging is in marketplace-id order and first key wins, so the floor is
//! deterministic; a key two marketplaces both set is logged rather than
//! silently resolved, because the two are asserting different things about the
//! same entity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_models::services::{MarketplaceConfig, MarketplaceMemberKind, ServicesConfig};

use super::types::EntityKind;

#[must_use]
pub fn member_attribute_floor(
    services: &ServicesConfig,
    kind: EntityKind,
    id: &str,
) -> Option<BTreeMap<String, serde_json::Value>> {
    let mut floor: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for config in services.enabled_marketplaces() {
        if config.access.attributes.is_empty() || !is_member(services, config, kind, id) {
            continue;
        }
        for (key, value) in &config.access.attributes {
            if let Some(kept) = floor.get(key) {
                if kept != value {
                    tracing::warn!(
                        marketplace = %config.id,
                        attribute = %key,
                        entity_id = id,
                        "marketplace floor: attribute already set by an earlier marketplace; \
                         keeping the first value in id order"
                    );
                }
                continue;
            }
            floor.insert(key.clone(), value.clone());
        }
    }
    (!floor.is_empty()).then_some(floor)
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
