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
        match self.services.marketplaces.len() {
            0 => None,
            1 => self.services.marketplaces.iter().next(),
            _ => {
                let id = self.services.settings.default_marketplace_id.as_ref()?;
                self.services.marketplaces.get_key_value(id)
            },
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
