//! Read-only access to the configured marketplaces.
//!
//! [`MarketplaceService`] borrows a [`ServicesConfig`] and resolves
//! marketplace lookups, the default-marketplace fallback, and referential
//! integrity without owning or cloning the config.

use std::collections::BTreeMap;

use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::{MarketplaceAccess, MarketplaceConfig, ServicesConfig};
use systemprompt_security::authz::EntityKind;

use crate::error::MarketplaceError;

/// Conventional marketplace id used when no `default_marketplace_id` is set.
const DEFAULT_MARKETPLACE_FALLBACK: &str = "default";

#[derive(Debug, Clone, Copy)]
pub struct MarketplaceService<'a> {
    services: &'a ServicesConfig,
}

impl<'a> MarketplaceService<'a> {
    #[must_use]
    pub const fn new(services: &'a ServicesConfig) -> Self {
        Self { services }
    }

    #[must_use]
    pub const fn list(&self) -> &'a std::collections::HashMap<MarketplaceId, MarketplaceConfig> {
        &self.services.marketplaces
    }

    pub fn get(&self, id: &MarketplaceId) -> Result<&'a MarketplaceConfig, MarketplaceError> {
        self.services
            .marketplaces
            .iter()
            .find(|(k, _)| k.as_str() == id.as_str())
            .map(|(_, v)| v)
            .ok_or_else(|| MarketplaceError::NotFound(id.clone()))
    }

    pub fn resolve_default(
        &self,
    ) -> Result<(&'a MarketplaceId, &'a MarketplaceConfig), MarketplaceError> {
        let id = self
            .services
            .settings
            .default_marketplace_id
            .clone()
            .or_else(|| {
                self.services
                    .marketplaces
                    .keys()
                    .any(|k| k.as_str() == DEFAULT_MARKETPLACE_FALLBACK)
                    .then(|| MarketplaceId::new(DEFAULT_MARKETPLACE_FALLBACK))
            })
            .ok_or(MarketplaceError::NoDefault)?;

        self.services
            .marketplaces
            .iter()
            .find(|(k, _)| k.as_str() == id.as_str())
            .ok_or(MarketplaceError::NoDefault)
    }

    /// Resolve the single active marketplace for manifest scoping.
    ///
    /// `None` means no scoping (global fallback). With several marketplaces
    /// configured the active one is named by `settings.default_marketplace_id`,
    /// which [`ServicesConfig::validate`] guarantees is present and resolvable
    /// whenever more than one marketplace is configured.
    #[must_use]
    pub fn active(&self) -> Option<&'a MarketplaceConfig> {
        match self.services.marketplaces.len() {
            0 => None,
            1 => self.services.marketplaces.values().next(),
            _ => {
                let id = self.services.settings.default_marketplace_id.as_ref()?;
                self.services
                    .marketplaces
                    .iter()
                    .find(|(k, _)| k.as_str() == id.as_str())
                    .map(|(_, v)| v)
            },
        }
    }

    /// Map every member of the active marketplace to its owning marketplace id.
    ///
    /// Keyed by `(EntityKind, member id)` over the active marketplace's
    /// `skills`/`agents`/`mcp_servers`/`plugins` include lists. An RBAC filter
    /// uses this to attribute a member to the marketplace whose grant governs
    /// it. With no active marketplace the map is empty.
    #[must_use]
    pub fn membership(&self) -> BTreeMap<(EntityKind, String), MarketplaceId> {
        let mut members = BTreeMap::new();
        let Some(config) = self.active() else {
            return members;
        };

        let kinds = [
            (EntityKind::Skill, &config.skills.include),
            (EntityKind::Agent, &config.agents.include),
            (EntityKind::McpServer, &config.mcp_servers.include),
            (EntityKind::Plugin, &config.plugins.include),
        ];
        for (kind, include) in kinds {
            for member in include {
                members.insert((kind, member.clone()), config.id.clone());
            }
        }
        members
    }

    /// Access block of the active marketplace, or `None` when none is active.
    #[must_use]
    pub fn active_access(&self) -> Option<&'a MarketplaceAccess> {
        self.active().map(|config| &config.access)
    }

    /// The active marketplace's required attribute floor for one member.
    ///
    /// Composes [`membership`](Self::membership) with
    /// [`active_access`](Self::active_access): returns the active marketplace's
    /// `access.attributes` when `(kind, id)` is a member and that bag is
    /// non-empty, else `None`. Core never interprets the values — the bag is
    /// forwarded verbatim to the ABAC hook as a defence-in-depth floor.
    #[must_use]
    pub fn member_attribute_floor(
        &self,
        kind: EntityKind,
        id: &str,
    ) -> Option<&'a BTreeMap<String, serde_json::Value>> {
        let access = self.active_access()?;
        if access.attributes.is_empty() {
            return None;
        }
        self.membership()
            .contains_key(&(kind, id.to_owned()))
            .then_some(&access.attributes)
    }

    pub fn validate_referential_integrity(&self) -> Result<(), MarketplaceError> {
        self.services
            .validate()
            .map_err(|e| MarketplaceError::Validation(e.to_string()))
    }
}
