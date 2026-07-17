//! Marketplace scoping for the bridge manifest.
//!
//! Intersects the on-disk catalogue lists with the active marketplace's
//! `MarketplaceConfig.<entity>.include` lists.
//!
//! Within an active marketplace, an empty `include:` list falls back to the
//! global list: validation rejects an `Explicit` ref with an empty include at
//! load time, so an empty list here means "all".
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::services::{MarketplaceConfig, ServicesConfig};

use crate::service::MarketplaceService;

#[must_use]
pub fn active_marketplace(services: &ServicesConfig) -> Option<&MarketplaceConfig> {
    MarketplaceService::new(services).active()
}

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
