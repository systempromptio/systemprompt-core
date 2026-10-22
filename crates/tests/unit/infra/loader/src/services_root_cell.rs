//! The process-wide services-root cell.
//!
//! The cell is installed once. Before installation `require` refuses rather
//! than guessing a root, and afterwards it serves the installed root.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use systemprompt_loader::services_root::{
    ActiveServicesRoot, ServicesProvenance, ServicesRootBootstrap,
};

#[test]
fn the_cell_refuses_before_installation_then_serves_the_installed_root() {
    assert!(!ServicesRootBootstrap::is_initialized());
    assert_eq!(ServicesRootBootstrap::get(), None);
    assert!(ServicesRootBootstrap::require().is_err());

    let installed = ServicesRootBootstrap::install(ActiveServicesRoot {
        path: PathBuf::from("/cache/composed/abc"),
        base: PathBuf::from("/services"),
        provenance: ServicesProvenance::Fetched {
            composed_hash: "abc".to_owned(),
            versions: BTreeMap::from([("base".to_owned(), "1.2.3".to_owned())]),
        },
    });

    assert_eq!(installed.path, PathBuf::from("/cache/composed/abc"));
    assert!(ServicesRootBootstrap::is_initialized());
    assert_eq!(
        ServicesRootBootstrap::require()
            .map(|root| root.path.clone())
            .ok(),
        Some(PathBuf::from("/cache/composed/abc"))
    );

    let second = ServicesRootBootstrap::install(ActiveServicesRoot {
        path: PathBuf::from("/cache/composed/def"),
        base: PathBuf::from("/services"),
        provenance: ServicesProvenance::Bundled,
    });

    assert_eq!(
        second.path,
        PathBuf::from("/cache/composed/abc"),
        "a second install never re-points a root other code already read"
    );
}

#[test]
fn provenance_records_the_error_that_forced_a_fallback() {
    let last_good = ServicesProvenance::LastGood {
        composed_hash: "abc".to_owned(),
        error: "registry unreachable".to_owned(),
    };
    let json = serde_json::to_value(&last_good).expect("serialise");

    assert_eq!(json["kind"], "last_good");
    assert_eq!(json["error"], "registry unreachable");
    assert_eq!(
        serde_json::from_value::<ServicesProvenance>(json).expect("round trip"),
        last_good
    );

    let bundled_fallback = serde_json::to_value(ServicesProvenance::BundledFallback {
        error: "signature refused".to_owned(),
    })
    .expect("serialise");
    assert_eq!(bundled_fallback["kind"], "bundled_fallback");
    assert_eq!(
        serde_json::to_value(ServicesProvenance::Bundled).expect("serialise")["kind"],
        "bundled"
    );
}
