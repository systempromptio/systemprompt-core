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
//! [`load_marketplace_parent`] loads the owning marketplace's ruleset and
//! `default_included` sentinel so it can cascade onto member entities as a
//! [`ResolveParent`]: one marketplace rule covers every member that declares
//! no rules of its own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::{MarketplaceConfig, MarketplaceMemberKind, ServicesConfig};

use super::error::AuthzResult;
use super::repository::AccessControlRepository;
use super::resolver::ResolveParent;
use super::types::{AccessRule, EntityKind, EntityRef};

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

/// The owning marketplace's ruleset and `default_included` sentinel, loaded
/// once and cascaded onto member entities via [`Self::as_resolve_parent`].
#[derive(Debug, Clone)]
pub struct MarketplaceParent {
    pub entity: EntityRef,
    pub rules: Vec<AccessRule>,
    pub default_included: Option<bool>,
}

impl MarketplaceParent {
    #[must_use]
    pub fn as_resolve_parent(&self) -> ResolveParent<'_> {
        ResolveParent {
            entity: &self.entity,
            rules: &self.rules,
            default_included: self.default_included,
        }
    }
}

pub async fn load_marketplace_parent(
    repo: &AccessControlRepository,
    marketplace_id: &MarketplaceId,
    fallback_default_included: Option<bool>,
) -> AuthzResult<MarketplaceParent> {
    let id = marketplace_id.as_str();
    let rules = repo
        .list_rules_for_entity(EntityKind::Marketplace, id)
        .await?;
    let default_included = repo
        .get_entity(EntityKind::Marketplace, id)
        .await?
        .map(|e| e.default_included)
        .or(fallback_default_included);
    Ok(MarketplaceParent {
        entity: EntityRef::Marketplace(marketplace_id.clone()),
        rules,
        default_included,
    })
}

fn active_marketplace(services: &ServicesConfig) -> Option<&MarketplaceConfig> {
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
