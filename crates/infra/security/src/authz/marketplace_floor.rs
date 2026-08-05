//! Marketplace membership attribute floor for authz enforcement sites.
//!
//! Resolves the active marketplace from a [`ServicesConfig`] and, for entities
//! included in it, exposes the declarative `access.attributes` bag. Core never
//! interprets the bag — it is forwarded verbatim to the ABAC hook (via
//! [`super::types::AuthzContext::with_marketplace_floor`]) as a
//! defence-in-depth floor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_models::services::{MarketplaceConfig, ServicesConfig};

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
    is_member(config, kind, id).then_some(&config.access.attributes)
}

fn active_marketplace(services: &ServicesConfig) -> Option<&MarketplaceConfig> {
    match services.marketplaces.len() {
        0 => None,
        1 => services.marketplaces.values().next(),
        _ => {
            let id = services.settings.default_marketplace_id.as_ref()?;
            services.marketplaces.get(id)
        },
    }
}

fn is_member(config: &MarketplaceConfig, kind: EntityKind, id: &str) -> bool {
    let include = match kind {
        EntityKind::Skill => &config.skills.include,
        EntityKind::Agent => &config.agents.include,
        EntityKind::McpServer => &config.mcp_servers.include,
        EntityKind::Plugin => &config.plugins.include,
        _ => return false,
    };
    include.iter().any(|member| member == id)
}
