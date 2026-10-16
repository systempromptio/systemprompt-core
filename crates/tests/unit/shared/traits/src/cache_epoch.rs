//! Cache invalidation epochs: labels round-trip and an advance is observed.

use systemprompt_traits::{CacheDomain, advance_cache_epoch, cache_epoch};

#[test]
fn every_domain_label_parses_back_to_itself() {
    for domain in CacheDomain::ALL {
        assert_eq!(CacheDomain::parse(domain.label()), Some(domain));
    }
    assert_eq!(CacheDomain::parse("unknown"), None);
}

#[test]
fn advancing_a_domain_moves_its_epoch_forward() {
    let before = cache_epoch(CacheDomain::Authz);
    let after = advance_cache_epoch(CacheDomain::Authz);
    assert!(after > before);
    assert!(cache_epoch(CacheDomain::Authz) >= after);
}
