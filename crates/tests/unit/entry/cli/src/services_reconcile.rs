//! Reconciling access rules after a `services refresh` swap.
//!
//! The database is only touched when the composition actually moved. Every
//! provenance that cannot name a new composed hash must return before a pool
//! is asked for, because `services refresh` runs on instances that have none.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::BTreeMap;

use systemprompt_cli::cli_settings::{CliConfig, OutputFormat};
use systemprompt_cli::context::CommandContext;
use systemprompt_cli::core::services::reconcile::reconcile_after_swap;
use systemprompt_cli::env_overrides::EnvOverrides;
use systemprompt_loader::bundle::BundleCache;
use systemprompt_loader::{ActiveServicesRoot, ServicesProvenance};
use systemprompt_models::services::bundle::ServicesBundleState;

use crate::services_profile_fixture as fx;

fn ctx() -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
}

async fn reconcile(provenance: ServicesProvenance, last_reconciled: Option<&str>) -> Vec<String> {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(dir.path().join("cache"));
    cache
        .write_state(&ServicesBundleState {
            composed_hash: "composed".to_owned(),
            last_reconciled_hash: last_reconciled.map(str::to_owned),
            sources: BTreeMap::new(),
        })
        .expect("state written");
    let root = ActiveServicesRoot {
        path: dir.path().join("services"),
        provenance,
    };
    let (_tree, profile) = fx::loaded(&fx::https_sources_block(&[]));
    reconcile_after_swap(&profile, &root, &cache, &ctx())
        .await
        .expect("nothing to reconcile")
        .into_iter()
        .map(|row| row.bundle)
        .collect()
}

#[tokio::test]
async fn a_bundled_tree_needs_no_reconciliation() {
    assert!(
        reconcile(ServicesProvenance::Bundled, None)
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn a_composition_already_reconciled_is_not_reprojected() {
    let rows = reconcile(
        ServicesProvenance::Fetched {
            composed_hash: "abc".to_owned(),
            versions: BTreeMap::new(),
        },
        Some("abc"),
    )
    .await;
    assert!(rows.is_empty(), "unexpected reconciliation: {rows:?}");
}

#[tokio::test]
async fn a_last_good_fallback_never_projects_the_stale_catalog() {
    let rows = reconcile(
        ServicesProvenance::LastGood {
            composed_hash: "abc".to_owned(),
            error: "origin down".to_owned(),
        },
        None,
    )
    .await;
    assert!(rows.is_empty(), "a fallback must not reconcile: {rows:?}");
}

#[tokio::test]
async fn a_bundled_fallback_never_projects_the_stale_catalog() {
    let rows = reconcile(
        ServicesProvenance::BundledFallback {
            error: "origin down".to_owned(),
        },
        None,
    )
    .await;
    assert!(rows.is_empty(), "a fallback must not reconcile: {rows:?}");
}

#[tokio::test]
async fn a_moved_composition_with_no_cached_fetch_state_refuses_to_guess() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(dir.path().join("cache"));
    cache
        .write_state(&ServicesBundleState::default())
        .expect("state written");
    let (_profile_tree, profile) = fx::loaded(&fx::https_sources_block(&[(
        "base",
        "https://example.test/bundle.tar.gz",
    )]));
    let root = ActiveServicesRoot {
        path: dir.path().join("services"),
        provenance: ServicesProvenance::Fetched {
            composed_hash: "abc".to_owned(),
            versions: BTreeMap::new(),
        },
    };

    let error = reconcile_after_swap(&profile, &root, &cache, &ctx())
        .await
        .expect_err("a bundle with no cached fetch state cannot be reconciled");
    let rendered = format!("{error:#}");
    assert!(rendered.contains("base"), "{rendered}");
    assert!(rendered.contains("cached fetch state"), "{rendered}");
}
