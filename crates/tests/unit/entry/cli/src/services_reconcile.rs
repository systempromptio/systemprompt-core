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
        base: dir.path().join("services"),
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
        base: dir.path().join("services"),
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

#[tokio::test]
async fn a_fetched_composition_is_projected_once_and_records_its_hash() {
    use chrono::Utc;
    use systemprompt_models::profile::ServicesSource;
    use systemprompt_models::services::bundle::{
        BundleOwnership, BundleSourceInfo, BundleSourceState, ServicesBundleManifest,
        SignedBundleManifest,
    };
    use systemprompt_runtime::DatabaseContext;
    use systemprompt_test_fixtures::DisposableDb;

    const SOURCE: &str = "cli-positive";
    const CONTENT_HASH: &str = "cli-content-hash";
    const COMPOSED_HASH: &str = "cli-composed-hash";

    let database = DisposableDb::installed("cli_services_reconcile")
        .await
        .expect("isolated migrated database");
    let pool = database.pool().await.expect("isolated database pool");
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let temp = tempfile::tempdir().expect("reconciliation fixture");
    let cache = BundleCache::new(temp.path().join("cache"));
    let mut sources = BTreeMap::new();
    sources.insert(
        SOURCE.to_owned(),
        BundleSourceState {
            digest: "sha256:cli".to_owned(),
            version: "1.0.0".to_owned(),
            content_hash: CONTENT_HASH.to_owned(),
            fetched_at: Utc::now(),
        },
    );
    cache
        .write_state(&ServicesBundleState {
            composed_hash: COMPOSED_HASH.to_owned(),
            last_reconciled_hash: None,
            sources,
        })
        .expect("seed pending composition state");
    let bundle_dir = cache.bundle_dir(SOURCE, CONTENT_HASH);
    std::fs::create_dir_all(&bundle_dir).unwrap();
    let signed = SignedBundleManifest {
        manifest: ServicesBundleManifest {
            format: 1,
            version: "1.0.0".to_owned(),
            created_at: Utc::now(),
            requires_core: ">=0.0.1".to_owned(),
            source: BundleSourceInfo::default(),
            files: Vec::new(),
            content_hash: CONTENT_HASH.to_owned(),
            total_size: 0,
            owns: BundleOwnership::default(),
        },
        signature: None,
    };
    std::fs::write(
        bundle_dir.join("bundle.json"),
        serde_json::to_vec_pretty(&signed).unwrap(),
    )
    .unwrap();

    let mut profile = systemprompt_config::ProfileBootstrap::get()
        .expect("bootstrap profile")
        .clone();
    profile.services.sources = vec![ServicesSource {
        name: SOURCE.to_owned(),
        https: None,
        oci: None,
    }];
    let root = ActiveServicesRoot {
        path: boot.services_path.clone(),
        base: boot.services_path.clone(),
        provenance: ServicesProvenance::Fetched {
            composed_hash: COMPOSED_HASH.to_owned(),
            versions: BTreeMap::new(),
        },
    };
    let command = CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        database.url().to_owned(),
    );

    let rows = reconcile_after_swap(&profile, &root, &cache, &command)
        .await
        .expect("project fetched composition");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].bundle, SOURCE);
    assert_eq!(rows[0].inserted, 0);
    assert_eq!(rows[0].updated, 0);
    assert_eq!(rows[0].deleted, 0);
    assert_eq!(rows[0].protected, 0);
    assert!(rows[0].inert_role_rules.is_empty());
    assert_eq!(
        cache.read_state().last_reconciled_hash.as_deref(),
        Some(COMPOSED_HASH),
        "successful projection durably acknowledges the exact composition"
    );

    drop(command);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    let closed = systemprompt_test_fixtures::closed_db_pool().await;
    let no_database_needed = CommandContext::with_database(
        CliConfig::new().with_interactive(false),
        EnvOverrides::default(),
        DatabaseContext::from_pool(closed),
        "postgresql://closed.invalid/test".to_owned(),
    );
    let repeated = reconcile_after_swap(&profile, &root, &cache, &no_database_needed)
        .await
        .expect("recorded composition is a no-op without database access");
    assert!(repeated.is_empty());
    drop(no_database_needed);
    database.drop_now().await;
}
