//! Read-only access to the configured marketplaces.
//!
//! [`MarketplaceService`] borrows a [`ServicesConfig`] and resolves marketplace
//! lookups, the active-marketplace selection, and referential integrity without
//! owning or cloning the config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::{MarketplaceConfig, ServicesConfig};

use crate::error::MarketplaceError;

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
            .get(id)
            .ok_or_else(|| MarketplaceError::NotFound(id.clone()))
    }

    pub fn resolve_default(
        &self,
    ) -> Result<(&'a MarketplaceId, &'a MarketplaceConfig), MarketplaceError> {
        self.active_entry().ok_or(MarketplaceError::NoDefault)
    }

    fn active_entry(&self) -> Option<(&'a MarketplaceId, &'a MarketplaceConfig)> {
        let mut enabled = self
            .services
            .marketplaces
            .iter()
            .filter(|(_, config)| config.enabled);
        let first = enabled.next()?;
        if enabled.next().is_none() {
            return Some(first);
        }
        let id = self.services.settings.default_marketplace_id.as_ref()?;
        self.services
            .marketplaces
            .get_key_value(id)
            .filter(|(_, config)| config.enabled)
    }

    // Why: with at least one enabled marketplace configured, an unresolvable
    // selection must fail closed rather than assemble unscoped.
    pub fn resolve_active(&self) -> Result<Option<&'a MarketplaceConfig>, MarketplaceError> {
        let any_enabled = self.services.marketplaces.values().any(|m| m.enabled);
        match self.active_entry() {
            Some((_, config)) => Ok(Some(config)),
            None if any_enabled => Err(MarketplaceError::NoDefault),
            None => Ok(None),
        }
    }

    #[must_use]
    pub fn active(&self) -> Option<&'a MarketplaceConfig> {
        self.active_entry().map(|(_, config)| config)
    }

    pub fn validate_referential_integrity(&self) -> Result<(), MarketplaceError> {
        self.services
            .validate()
            .map_err(|e| MarketplaceError::Validation(e.to_string()))
    }
}
