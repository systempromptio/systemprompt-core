//! The services root a boot context holds.
//!
//! The cell is installed once. Before installation `services_root` is empty and
//! `require_services_root` serves the baked root, and afterwards both serve the
//! installed root.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use systemprompt_loader::services_root::{ActiveServicesRoot, ServicesProvenance};
use systemprompt_test_fixtures::ensure_test_bootstrap;

#[test]
fn the_cell_serves_the_baked_root_before_installation_then_the_installed_root() {
    let boot = ensure_test_bootstrap().boot.clone();
    assert_eq!(boot.services_root(), None);
    let baked = boot.require_services_root();
    assert_eq!(baked, ActiveServicesRoot::baked(boot.profile()));

    let installed = boot.install_services_root(ActiveServicesRoot {
        path: PathBuf::from("/cache/composed/abc"),
        base: PathBuf::from("/services"),
        provenance: ServicesProvenance::Fetched {
            composed_hash: "abc".to_owned(),
            versions: BTreeMap::from([("base".to_owned(), "1.2.3".to_owned())]),
        },
    });

    assert_eq!(installed.path, PathBuf::from("/cache/composed/abc"));
    assert_eq!(
        boot.require_services_root().path,
        PathBuf::from("/cache/composed/abc")
    );

    let second = boot.install_services_root(ActiveServicesRoot {
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
