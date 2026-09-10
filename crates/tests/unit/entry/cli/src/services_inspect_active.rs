//! `core services inspect --active` reports the provenance of the composition
//! the instance is actually running.
//!
//! Each case installs the process-global profile and services-root singletons,
//! which nextest's process-per-test model makes safe.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::BTreeMap;

use chrono::Utc;
use systemprompt_cli::core::services::inspect::{InspectArgs, execute};
use systemprompt_config::ProfileBootstrap;
use systemprompt_loader::bundle::{BundleCache, cache_root};
use systemprompt_loader::{ActiveServicesRoot, ServicesProvenance, ServicesRootBootstrap};
use systemprompt_models::services::bundle::{BundleSourceState, ServicesBundleState};

use crate::services_profile_fixture as fx;

fn install(provenance: ServicesProvenance) -> fx::ProfileTree {
    let tree = fx::write_tree(
        &fx::https_sources_block(&[("base", "https://example.test/bundle.tar.gz")]),
        "secrets:\n  secrets_path: secrets.json\n  source: env\n",
    );
    ProfileBootstrap::init_from_path(&tree.profile_path).expect("profile installs");
    let profile = ProfileBootstrap::get().expect("profile");
    let mut sources = BTreeMap::new();
    sources.insert(
        "base".to_owned(),
        BundleSourceState {
            digest: "sha256:aaa".to_owned(),
            version: "1.2.3".to_owned(),
            content_hash: "hash".to_owned(),
            fetched_at: Utc::now(),
        },
    );
    BundleCache::new(cache_root(profile))
        .write_state(&ServicesBundleState {
            composed_hash: "composed".to_owned(),
            last_reconciled_hash: Some("reconciled".to_owned()),
            sources,
        })
        .expect("state written");
    ServicesRootBootstrap::install(ActiveServicesRoot {
        path: tree.root.join("services"),
        provenance,
    });
    tree
}

fn rendered() -> String {
    let output = execute(&InspectArgs {
        bundle: None,
        active: true,
    })
    .expect("active inspect succeeds");
    serde_json::to_string(output.artifact()).expect("output serialises")
}

#[test]
fn a_fetched_composition_reports_its_hash_and_source_versions() {
    let _tree = install(ServicesProvenance::Fetched {
        composed_hash: "composed-abc".to_owned(),
        versions: BTreeMap::new(),
    });
    let body = rendered();
    assert!(body.contains("fetched"), "{body}");
    assert!(body.contains("composed-abc"), "{body}");
    assert!(body.contains("base=1.2.3 (sha256:aaa)"), "{body}");
    assert!(body.contains("reconciled"), "{body}");
}

#[test]
fn a_last_good_composition_carries_the_error_that_forced_it() {
    let _tree = install(ServicesProvenance::LastGood {
        composed_hash: "composed-old".to_owned(),
        error: "origin timed out".to_owned(),
    });
    let body = rendered();
    assert!(body.contains("last_good"), "{body}");
    assert!(body.contains("origin timed out"), "{body}");
}

#[test]
fn a_bundled_fallback_reports_no_composed_hash() {
    let _tree = install(ServicesProvenance::BundledFallback {
        error: "no source resolved".to_owned(),
    });
    let body = rendered();
    assert!(body.contains("bundled_fallback"), "{body}");
    assert!(body.contains("no source resolved"), "{body}");
}

#[test]
fn a_bundled_tree_reports_neither_a_hash_nor_an_error() {
    let _tree = install(ServicesProvenance::Bundled);
    let body = rendered();
    assert!(body.contains("bundled"), "{body}");
    assert!(!body.contains("bundled_fallback"), "{body}");
}
