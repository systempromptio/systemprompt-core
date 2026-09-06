//! Read-only access to the configured marketplaces.
//!
//! [`MarketplaceService`] borrows a [`ServicesConfig`] and resolves marketplace
//! lookups, the enabled set, the rendering default, and referential integrity
//! without owning or cloning the config.
//!
//! There is no "active" marketplace: the manifest unions every enabled one.
//! [`MarketplaceService::resolve_default`] selects the single marketplace the
//! public `/marketplace.json` and the CLI's generated file render, and nothing
//! else.
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
        self.default_entry().ok_or(MarketplaceError::NoDefault)
    }

    #[must_use]
    pub fn enabled(&self) -> Vec<(&'a MarketplaceId, &'a MarketplaceConfig)> {
        let mut out: Vec<(&'a MarketplaceId, &'a MarketplaceConfig)> = self
            .services
            .marketplaces
            .iter()
            .filter(|(_, config)| config.enabled)
            .collect();
        out.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));
        out
    }

    fn default_entry(&self) -> Option<(&'a MarketplaceId, &'a MarketplaceConfig)> {
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

    pub fn validate_referential_integrity(&self) -> Result<(), MarketplaceError> {
        self.services
            .validate()
            .map_err(|e| MarketplaceError::Validation(e.to_string()))
    }
}
