use systemprompt_identifiers::MarketplaceId;
use systemprompt_marketplace::{MarketplaceError, MarketplaceFilterError};
use systemprompt_security::ManifestSigningError;

#[test]
fn filter_error_backend_display() {
    let e = MarketplaceFilterError::Backend("db offline".into());
    assert!(e.to_string().contains("db offline"));
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
fn marketplace_error_catalog_display() {
    let e = MarketplaceError::Catalog("read failed".into());
    assert!(e.to_string().contains("read failed"));
}

#[test]
fn marketplace_error_signing_display() {
    let e = MarketplaceError::Signing(ManifestSigningError::KeyMissing);
    assert!(e.to_string().contains("signing key missing"));
    assert!(std::error::Error::source(&e).is_some());
}

#[test]
fn marketplace_error_catalog_source_keeps_cause() {
    let cause = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
    let e = MarketplaceError::catalog("read skills", cause);
    assert!(matches!(e, MarketplaceError::CatalogSource { .. }));
    assert!(e.to_string().contains("read skills"));
    let source = std::error::Error::source(&e).expect("cause is kept");
    assert!(source.to_string().contains("no such file"));
}

#[test]
fn marketplace_error_import_source_keeps_cause() {
    let cause = std::io::Error::other("disk gone");
    let e = MarketplaceError::import(std::path::Path::new("/tmp/kit"), "read", cause);
    assert!(matches!(e, MarketplaceError::ImportSource { ref path, .. } if path == "/tmp/kit"));
    assert!(std::error::Error::source(&e).is_some());
}

#[test]
fn marketplace_error_from_filter_error() {
    let fe = MarketplaceFilterError::Backend("upstream down".into());
    let me = MarketplaceError::from(fe);
    assert!(matches!(me, MarketplaceError::Filter(_)));
    assert!(me.to_string().contains("upstream down"));
}
