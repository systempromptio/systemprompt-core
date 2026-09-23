//! The per-user resolved memo: keyed on disk fingerprint, user and managed
//! stamp; one entry per user; bounded; expired after the TTL.

use std::sync::Arc;
use std::time::{Duration, Instant};

use systemprompt_identifiers::UserId;
use systemprompt_marketplace::{
    CatalogContent, MarketplaceCache, MarketplaceCandidate, RESOLVED_CAPACITY, RESOLVED_TTL,
    ResolvedCatalog, ResolvedKey,
};
use systemprompt_models::services::ServicesConfig;

fn resolved(plugins: usize) -> Arc<ResolvedCatalog> {
    let dir = tempfile::tempdir().expect("services root");
    let catalog = CatalogContent::load(
        &ServicesConfig::default(),
        dir.path(),
        "https://api.example.invalid",
    )
    .expect("empty catalogue loads");
    let candidate = MarketplaceCandidate {
        plugins: (0..plugins)
            .map(|n| crate::plugin(&format!("plugin-{n}")))
            .collect(),
        ..MarketplaceCandidate::default()
    };
    Arc::new(ResolvedCatalog {
        catalog: Arc::new(catalog),
        candidate: Arc::new(candidate),
        bundles: Arc::new(Default::default()),
    })
}

fn key(user: &str, stamp: &str, catalog: u8) -> ResolvedKey {
    ResolvedKey {
        catalog: [catalog; 32],
        user: UserId::new(user.to_owned()),
        managed_stamp: stamp.to_owned(),
    }
}

#[test]
fn a_hit_needs_every_key_field_to_match() {
    let cache = MarketplaceCache::default();
    cache.store_resolved(key("ada", "s1", 1), resolved(1));
    assert!(cache.resolved(&key("ada", "s1", 1)).is_some());
    assert!(
        cache.resolved(&key("ada", "s2", 1)).is_none(),
        "managed stamp moved"
    );
    assert!(
        cache.resolved(&key("ada", "s1", 2)).is_none(),
        "disk fingerprint moved"
    );
    assert!(
        cache.resolved(&key("bob", "s1", 1)).is_none(),
        "another user"
    );
}

#[test]
fn users_keep_their_own_entries_and_a_user_keeps_only_one() {
    let cache = MarketplaceCache::default();
    cache.store_resolved(key("ada", "s1", 1), resolved(1));
    cache.store_resolved(key("bob", "s1", 1), resolved(2));
    assert_eq!(
        cache
            .resolved(&key("ada", "s1", 1))
            .expect("ada")
            .candidate
            .plugins
            .len(),
        1
    );
    assert_eq!(
        cache
            .resolved(&key("bob", "s1", 1))
            .expect("bob")
            .candidate
            .plugins
            .len(),
        2
    );

    cache.store_resolved(key("ada", "s2", 1), resolved(3));
    assert!(
        cache.resolved(&key("ada", "s1", 1)).is_none(),
        "the stale key is replaced"
    );
    assert_eq!(
        cache
            .resolved(&key("ada", "s2", 1))
            .expect("ada again")
            .candidate
            .plugins
            .len(),
        3
    );
}

#[test]
fn an_entry_expires_after_the_ttl_even_when_its_key_matches() {
    let cache = MarketplaceCache::default();
    let built = Instant::now();
    cache.store_resolved_at(key("ada", "s1", 1), resolved(1), built);
    assert!(
        cache
            .resolved_at(
                &key("ada", "s1", 1),
                built + RESOLVED_TTL - Duration::from_millis(1)
            )
            .is_some()
    );
    assert!(
        cache
            .resolved_at(&key("ada", "s1", 1), built + RESOLVED_TTL)
            .is_none()
    );
}

#[test]
fn the_oldest_user_is_evicted_past_capacity() {
    let cache = MarketplaceCache::default();
    for n in 0..=RESOLVED_CAPACITY {
        cache.store_resolved(key(&format!("user-{n}"), "s", 1), resolved(1));
    }
    for n in 1..=RESOLVED_CAPACITY {
        assert!(cache.resolved(&key(&format!("user-{n}"), "s", 1)).is_some());
    }
    assert!(cache.resolved(&key("user-0", "s", 1)).is_none());
    assert!(
        cache
            .resolved(&key(&format!("user-{RESOLVED_CAPACITY}"), "s", 1))
            .is_some()
    );
}
