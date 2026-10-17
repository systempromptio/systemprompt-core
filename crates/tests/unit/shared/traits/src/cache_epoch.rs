//! Cache invalidation epochs: labels round-trip and an advance is observed.

use systemprompt_traits::{CacheDomain, CacheEpochs};

#[test]
fn every_domain_label_parses_back_to_itself() {
    for domain in CacheDomain::ALL {
        assert_eq!(CacheDomain::parse(domain.label()), Some(domain));
    }
    assert_eq!(CacheDomain::parse("unknown"), None);
}

#[test]
fn advancing_a_domain_moves_only_its_epoch_forward() {
    let epochs = CacheEpochs::new();
    let after = epochs.advance(CacheDomain::Authz);
    assert_eq!(after, 1);
    assert_eq!(epochs.current(CacheDomain::Authz), 1);
    assert_eq!(epochs.current(CacheDomain::JtiRevocation), 0);
}

#[test]
fn clones_share_epochs_and_separate_handles_do_not() {
    let epochs = CacheEpochs::new();
    let shared = epochs.clone();
    let other = CacheEpochs::new();
    shared.advance_all();
    for domain in CacheDomain::ALL {
        assert_eq!(epochs.current(domain), 1);
        assert_eq!(other.current(domain), 0);
    }
}
