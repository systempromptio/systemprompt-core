//! Marketplace scoping for the bridge manifest.
//!
//! Intersects the on-disk catalogue lists with the active marketplace's
//! `MarketplaceConfig.<entity>.include` lists.
//!
//! Within an active marketplace, an empty `include:` list falls back to the
//! global list: validation rejects an `Explicit` ref with an empty include at
//! load time, so an empty list here means "all".

use systemprompt_models::services::{MarketplaceConfig, ServicesConfig};

use crate::service::MarketplaceService;

/// Resolve the active marketplace for manifest scoping.
///
/// `None` means no scoping (global fallback). With multiple marketplaces
/// configured the active one is named by `settings.default_marketplace_id`,
/// which [`ServicesConfig::validate`] requires whenever more than one is
/// present — so this never picks ambiguously.
#[must_use]
pub fn active_marketplace(services: &ServicesConfig) -> Option<&MarketplaceConfig> {
    MarketplaceService::new(services).active()
}

/// Filter `items` to those whose id (per `id_of`) appears in `include`.
///
/// An empty `include` is the global-list fallback and returns `items`
/// unchanged. Preserves the on-disk order of `items`.
pub fn scope_to_marketplace<T, F>(items: Vec<T>, include: &[String], id_of: F) -> Vec<T>
where
    F: Fn(&T) -> &str,
{
    if include.is_empty() {
        return items;
    }
    items
        .into_iter()
        .filter(|item| include.iter().any(|inc| inc == id_of(item)))
        .collect()
}
