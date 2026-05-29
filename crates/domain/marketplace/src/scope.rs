//! Marketplace scoping for the bridge manifest.
//!
//! Intersects the on-disk catalogue lists with the active marketplace's
//! `MarketplaceConfig.<entity>.include` lists.
//!
//! Within an active marketplace, an empty `include:` list falls back to the
//! global list: validation rejects an `Explicit` ref with an empty include at
//! load time, so an empty list here means "all".

use systemprompt_models::services::{MarketplaceConfig, ServicesConfig};

/// Resolve the active marketplace for manifest scoping.
///
/// `None` means no scoping (global fallback). With multiple marketplaces
/// configured this picks one and warns: fail-open is intentional until a
/// profile-level selector exists.
#[must_use]
pub fn active_marketplace(services: &ServicesConfig) -> Option<&MarketplaceConfig> {
    match services.marketplaces.len() {
        0 => None,
        1 => services.marketplaces.values().next(),
        n => {
            tracing::warn!(
                count = n,
                "bridge_manifest: multiple marketplaces configured without a profile selector; \
                 picking the first by HashMap iteration order. Follow-up: add \
                 Profile::active_marketplace_id and fail closed on ambiguity."
            );
            services.marketplaces.values().next()
        },
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Item {
        id: &'static str,
    }

    #[test]
    fn empty_include_returns_input_unchanged() {
        let items = vec![Item { id: "a" }, Item { id: "b" }];
        let out = scope_to_marketplace(items, &[], |i| i.id);
        assert_eq!(out, vec![Item { id: "a" }, Item { id: "b" }]);
    }

    #[test]
    fn include_filters_to_listed_ids_preserving_order() {
        let items = vec![Item { id: "a" }, Item { id: "b" }, Item { id: "c" }];
        let include = vec!["c".to_owned(), "a".to_owned()];
        let out = scope_to_marketplace(items, &include, |i| i.id);
        assert_eq!(out, vec![Item { id: "a" }, Item { id: "c" }]);
    }

    #[test]
    fn nonexistent_include_entries_are_dropped_silently() {
        let items = vec![Item { id: "a" }, Item { id: "b" }];
        let include = vec!["a".to_owned(), "ghost".to_owned()];
        let out = scope_to_marketplace(items, &include, |i| i.id);
        assert_eq!(out, vec![Item { id: "a" }]);
    }
}
