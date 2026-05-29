//! Read-only access to the configured marketplaces.
//!
//! [`MarketplaceService`] borrows a [`ServicesConfig`] and resolves
//! marketplace lookups, the default-marketplace fallback, and referential
//! integrity without owning or cloning the config.

use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::{MarketplaceConfig, ServicesConfig};

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
    pub fn list(&self) -> &'a std::collections::HashMap<MarketplaceId, MarketplaceConfig> {
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
                    .then(|| DEFAULT_MARKETPLACE_FALLBACK.to_owned())
            })
            .ok_or(MarketplaceError::NoDefault)?;

        self.services
            .marketplaces
            .iter()
            .find(|(k, _)| k.as_str() == id)
            .ok_or(MarketplaceError::NoDefault)
    }

    /// Resolve the single active marketplace for manifest scoping.
    ///
    /// `None` means no scoping (global fallback). With several marketplaces
    /// configured this picks one by iteration order and warns: fail-open is
    /// intentional until a profile-level selector exists.
    #[must_use]
    pub fn active(&self) -> Option<&'a MarketplaceConfig> {
        match self.services.marketplaces.len() {
            0 => None,
            1 => self.services.marketplaces.values().next(),
            n => {
                tracing::warn!(
                    count = n,
                    "marketplace: multiple marketplaces configured without a profile selector; \
                     picking the first by HashMap iteration order"
                );
                self.services.marketplaces.values().next()
            },
        }
    }

    pub fn validate_referential_integrity(&self) -> Result<(), MarketplaceError> {
        self.services
            .validate()
            .map_err(|e| MarketplaceError::Validation(e.to_string()))
    }
}
