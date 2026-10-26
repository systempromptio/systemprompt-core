//! `core services inspect --active` reports the provenance of the composition
//! the instance is actually running.
//!
//! Each case builds its own boot context over a tempdir profile and installs
//! the active services root on it.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::BTreeMap;

use chrono::Utc;
use systemprompt_cli::CommandContext;
use systemprompt_cli::core::services::inspect::{InspectArgs, execute};
use systemprompt_loader::bundle::{BundleCache, cache_root};
use systemprompt_manifest::services::bundle::{BundleSourceState, ServicesBundleState};
use systemprompt_manifest::services::root::{ActiveServicesRoot, ServicesProvenance};

use crate::services_profile_fixture as fx;

fn install(provenance: ServicesProvenance) -> (fx::ProfileTree, CommandContext) {
    let tree = fx::write_tree(
        &fx::https_sources_block(&[("base", "https://example.test/bundle.tar.gz")]),
        "secrets:\n  secrets_path: secrets.json\n  source: env\n",
    );
    let boot = fx::boot_for(&tree);
    let profile = boot.profile();
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
    boot.install_services_root(ActiveServicesRoot {
        path: tree.root.join("services"),
        base: tree.root.join("services"),
        provenance,
    });
    let ctx = fx::ctx_for(&boot);
    (tree, ctx)
}

fn rendered(ctx: &CommandContext) -> String {
    let output = execute(
        &InspectArgs {
            bundle: None,
            active: true,
        },
        ctx,
    )
    .expect("active inspect succeeds");
    serde_json::to_string(output.artifact()).expect("output serialises")
}

#[test]
fn a_fetched_composition_reports_its_hash_and_source_versions() {
    let (_tree, ctx) = install(ServicesProvenance::Fetched {
        composed_hash: "composed-abc".to_owned(),
        versions: BTreeMap::new(),
    });
    let body = rendered(&ctx);
    assert!(body.contains("fetched"), "{body}");
    assert!(body.contains("composed-abc"), "{body}");
    assert!(body.contains("base=1.2.3 (sha256:aaa)"), "{body}");
    assert!(body.contains("reconciled"), "{body}");
}

#[test]
fn a_last_good_composition_carries_the_error_that_forced_it() {
    let (_tree, ctx) = install(ServicesProvenance::LastGood {
        composed_hash: "composed-old".to_owned(),
        error: "origin timed out".to_owned(),
    });
    let body = rendered(&ctx);
    assert!(body.contains("last_good"), "{body}");
    assert!(body.contains("origin timed out"), "{body}");
}

#[test]
fn a_bundled_fallback_reports_no_composed_hash() {
    let (_tree, ctx) = install(ServicesProvenance::BundledFallback {
        error: "no source resolved".to_owned(),
    });
    let body = rendered(&ctx);
    assert!(body.contains("bundled_fallback"), "{body}");
    assert!(body.contains("no source resolved"), "{body}");
}

#[test]
fn a_bundled_tree_reports_neither_a_hash_nor_an_error() {
    let (_tree, ctx) = install(ServicesProvenance::Bundled);
    let body = rendered(&ctx);
    assert!(body.contains("bundled"), "{body}");
    assert!(!body.contains("bundled_fallback"), "{body}");
}
