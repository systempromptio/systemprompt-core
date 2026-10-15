//! A `no-cache` manifest request reuses a resolution of the same key built
//! within [`FRESH_REVALIDATE_AFTER`] and rebuilds only an older one.
//!
//! A 0.67 bridge sends `no-cache` on every registry probe tick; rebuilding on
//! each one starved the database pool. The request looks for an entry built
//! at or after `now - FRESH_REVALIDATE_AFTER`, exactly as `resolve_for_user`
//! does before it takes the rebuild gate.

use std::sync::Arc;
use std::time::{Duration, Instant};

use systemprompt_api::routes::gateway::bridge_resolved::FRESH_REVALIDATE_AFTER;
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_marketplace::{
    CatalogContent, MarketplaceCache, MarketplaceCandidate, ResolvedCatalog, ResolvedKey,
};

fn resolved() -> Arc<ResolvedCatalog> {
    let dir = tempfile::tempdir().expect("services root");
    let catalog = CatalogContent::load(
        &ServicesConfig::default(),
        dir.path(),
        "https://api.example.invalid",
    )
    .expect("empty catalogue loads");
    Arc::new(ResolvedCatalog {
        catalog: Arc::new(catalog),
        candidate: Arc::new(MarketplaceCandidate::default()),
        bundles: Arc::new(Default::default()),
    })
}

fn key() -> ResolvedKey {
    ResolvedKey {
        catalog: [7; 32],
        user: UserId::new("fresh-reuse".to_owned()),
        managed_stamp: "stamp".to_owned(),
    }
}

fn fresh_hit(cache: &MarketplaceCache, now: Instant) -> Option<Arc<ResolvedCatalog>> {
    now.checked_sub(FRESH_REVALIDATE_AFTER)
        .and_then(|since| cache.resolved_built_since(&key(), since))
}

#[test]
fn the_window_is_thirty_seconds() {
    assert_eq!(FRESH_REVALIDATE_AFTER, Duration::from_secs(30));
}

#[test]
fn fresh_within_the_window_reuses_the_resolution() {
    let cache = MarketplaceCache::default();
    let now = Instant::now() + Duration::from_secs(120);
    let value = resolved();
    cache.store_resolved_at(key(), Arc::clone(&value), now - Duration::from_secs(10));
    let hit = fresh_hit(&cache, now).expect("a 10 s old resolution answers no-cache");
    assert!(Arc::ptr_eq(&hit, &value));
}

#[test]
fn fresh_after_the_window_rebuilds() {
    let cache = MarketplaceCache::default();
    let now = Instant::now() + Duration::from_secs(120);
    cache.store_resolved_at(key(), resolved(), now - Duration::from_secs(31));
    assert!(
        fresh_hit(&cache, now).is_none(),
        "a resolution older than the window is rebuilt"
    );
}

#[test]
fn fresh_never_reuses_another_key() {
    let cache = MarketplaceCache::default();
    let now = Instant::now() + Duration::from_secs(120);
    let mut moved = key();
    moved.managed_stamp = "moved".to_owned();
    cache.store_resolved_at(moved, resolved(), now - Duration::from_secs(1));
    assert!(fresh_hit(&cache, now).is_none());
}
