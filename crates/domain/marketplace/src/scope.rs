//! Marketplace scoping for the bridge manifest.
//!
//! Intersects the on-disk catalogue lists with the union of every enabled
//! marketplace's `MarketplaceConfig.<entity>.include` list. An entry any
//! enabled marketplace names survives; who may then see it is the authz
//! cascade's decision, not this one's.
//!
//! An empty `include:` list falls back to the global list: validation rejects
//! an `Explicit` ref with an empty include at load time, so an empty list means
//! "all" — and one such marketplace makes the whole union "all".
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_models::services::{MarketplaceConfig, MarketplaceMemberKind, ServicesConfig};

#[must_use]
pub fn enabled_marketplaces(services: &ServicesConfig) -> Vec<&MarketplaceConfig> {
    services.enabled_marketplaces()
}

#[must_use]
pub fn union_include(
    marketplaces: &[&MarketplaceConfig],
    kind: MarketplaceMemberKind,
) -> Option<BTreeSet<String>> {
    if marketplaces.is_empty() {
        return None;
    }
    let mut union = BTreeSet::new();
    for marketplace in marketplaces {
        let include = &marketplace.members(kind).include;
        if include.is_empty() {
            return None;
        }
        union.extend(include.iter().cloned());
    }
    Some(union)
}

pub fn scope_to_union<T, F>(items: Vec<T>, include: Option<&BTreeSet<String>>, id_of: F) -> Vec<T>
where
    F: Fn(&T) -> &str,
{
    match include {
        None => items,
        Some(include) => items
            .into_iter()
            .filter(|item| include.contains(id_of(item)))
            .collect(),
    }
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
