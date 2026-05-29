use systemprompt_identifiers::MarketplaceId;
use systemprompt_marketplace::{MarketplaceError, MarketplaceFilterError};

#[test]
fn filter_error_backend_display() {
    let e = MarketplaceFilterError::Backend("db offline".into());
    assert!(e.to_string().contains("db offline"));
}

#[test]
fn filter_error_unknown_user_display() {
    let e = MarketplaceFilterError::UnknownUser("uid-999".into());
    assert!(e.to_string().contains("uid-999"));
}

#[test]
fn filter_error_policy_display() {
    let e = MarketplaceFilterError::Policy("denied by rule".into());
    assert!(e.to_string().contains("denied by rule"));
}

#[test]
fn marketplace_error_not_found_display() {
    let e = MarketplaceError::NotFound(MarketplaceId::new("no-such-market"));
    assert!(e.to_string().contains("no-such-market"));
}

#[test]
fn marketplace_error_no_default_display() {
    let e = MarketplaceError::NoDefault;
    assert!(!e.to_string().is_empty());
}

#[test]
fn marketplace_error_validation_display() {
    let e = MarketplaceError::Validation("bad id chars".into());
    assert!(e.to_string().contains("bad id chars"));
}

#[test]
fn marketplace_error_catalog_display() {
    let e = MarketplaceError::Catalog("read failed".into());
    assert!(e.to_string().contains("read failed"));
}

#[test]
fn marketplace_error_signing_display() {
    let e = MarketplaceError::Signing("key not loaded".into());
    assert!(e.to_string().contains("key not loaded"));
}

#[test]
fn marketplace_error_from_filter_error() {
    let fe = MarketplaceFilterError::Backend("upstream down".into());
    let me = MarketplaceError::from(fe);
    assert!(matches!(me, MarketplaceError::Filter(_)));
    assert!(me.to_string().contains("upstream down"));
}
