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

#[test]
fn a_config_plane_notification_names_its_domain() {
    let planes = [
        ("config:gateway_routes", CacheDomain::GatewayRoutes),
        ("config:governance", CacheDomain::Governance),
        ("config:groups", CacheDomain::Groups),
        ("config:access_control", CacheDomain::AccessControl),
        ("config:gateway_policies", CacheDomain::GatewayPolicies),
    ];
    for (payload, domain) in planes {
        assert_eq!(CacheDomain::from_notification(payload), Some(domain));
        assert_eq!(domain.config_plane(), payload.strip_prefix("config:"));
    }
    assert_eq!(
        CacheDomain::from_notification("authz"),
        Some(CacheDomain::Authz)
    );
    assert_eq!(CacheDomain::from_notification("config:authz"), None);
    assert_eq!(CacheDomain::from_notification("config:unknown"), None);
    assert_eq!(CacheDomain::Authz.config_plane(), None);
}

#[tokio::test]
async fn a_subscriber_wakes_on_its_domain_only() {
    let epochs = CacheEpochs::new();
    let mut routes = epochs.subscribe(CacheDomain::GatewayRoutes);
    let groups = epochs.subscribe(CacheDomain::Groups);
    epochs.advance(CacheDomain::GatewayRoutes);
    routes.changed().await.expect("sender alive");
    assert_eq!(*routes.borrow_and_update(), 1);
    assert!(!groups.has_changed().expect("sender alive"));
}
