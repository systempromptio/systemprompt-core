//! A reconcile that fails after the composition was recorded is retried by the
//! next refresh and acknowledged once it succeeds.
//!
//! `state.json` is written when the composition is swapped in, before the
//! reconcile runs, so a failure leaves the source digests current. Without the
//! pending check the next refresh read "unchanged" and never reconciled.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::BTreeMap;

use chrono::Utc;
use systemprompt_cli::cli_settings::{CliConfig, OutputFormat};
use systemprompt_cli::context::CommandContext;
use systemprompt_cli::core::services::reconcile::reconcile_after_swap;
use systemprompt_cli::core::services::refresh::{
    RefreshOutcome, diff_states, outcome_with_pending,
};
use systemprompt_cli::env_overrides::EnvOverrides;
use systemprompt_loader::bundle::BundleCache;
use systemprompt_manifest::profile::ServicesSource;
use systemprompt_manifest::services::bundle::{
    BundleOwnership, BundleSourceInfo, BundleSourceState, ServicesBundleManifest,
    ServicesBundleState, SignedBundleManifest,
};
use systemprompt_manifest::services::root::{ActiveServicesRoot, ServicesProvenance};
use systemprompt_runtime::DatabaseContext;
use systemprompt_runtime::services_reconcile::pending_composed_hash;
use systemprompt_test_fixtures::{DisposableDb, ensure_test_bootstrap};

const SOURCE: &str = "retry-source";
const CONTENT_HASH: &str = "retry-content-hash";
const COMPOSED_HASH: &str = "retry-composed-hash";

fn pending(root: &ActiveServicesRoot, cache: &BundleCache) -> bool {
    pending_composed_hash(root, cache.read_state().last_reconciled_hash.as_deref()).is_some()
}

#[tokio::test]
async fn a_failed_reconcile_is_retried_by_the_next_refresh() {
    let database = DisposableDb::with_schema("cli_reconcile_retry").await;
    let pool = database.test_pool().await;
    let bootstrap = ensure_test_bootstrap();
    let temp = tempfile::tempdir().expect("fixture dir");
    let cache = BundleCache::new(temp.path().join("cache"));
    let source_state = BundleSourceState {
        digest: "sha256:retry".to_owned(),
        version: "1.0.0".to_owned(),
        content_hash: CONTENT_HASH.to_owned(),
        fetched_at: Utc::now(),
    };
    let state = ServicesBundleState {
        composed_hash: COMPOSED_HASH.to_owned(),
        last_reconciled_hash: None,
        sources: BTreeMap::from([(SOURCE.to_owned(), source_state)]),
    };
    cache
        .write_state(&state)
        .expect("state as the swap leaves it");
    let bundle_dir = cache.bundle_dir(SOURCE, CONTENT_HASH);
    std::fs::create_dir_all(&bundle_dir).expect("bundle dir");
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
        serde_json::to_vec_pretty(&signed).expect("manifest json"),
    )
    .expect("write manifest");

    let services = temp.path().join("services");
    std::fs::create_dir_all(services.join("config")).expect("services dir");
    std::fs::write(services.join("config/config.yaml"), "includes: [unclosed\n")
        .expect("broken composition");
    let root = ActiveServicesRoot {
        path: services.clone(),
        base: services.clone(),
        provenance: ServicesProvenance::Fetched {
            composed_hash: COMPOSED_HASH.to_owned(),
            versions: BTreeMap::new(),
        },
    };
    let mut profile = bootstrap.boot.profile().clone();
    profile.services.sources = vec![ServicesSource {
        name: SOURCE.to_owned(),
        https: None,
        oci: None,
    }];
    let command = CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        database.url().to_owned(),
    );

    reconcile_after_swap(&profile, &root, &cache, &command)
        .await
        .expect_err("the first reconcile fails");
    assert_eq!(cache.read_state().last_reconciled_hash, None);
    let unchanged_rows = diff_states(&state, &cache.read_state());
    assert_eq!(
        outcome_with_pending(&unchanged_rows, pending(&root, &cache)),
        RefreshOutcome::Changed,
        "the digests did not move, yet the refresh must still report the unfinished reconcile"
    );

    std::fs::write(services.join("config/config.yaml"), "includes: []\n").expect("repair");
    reconcile_after_swap(&profile, &root, &cache, &command)
        .await
        .expect("the retry succeeds");
    assert_eq!(
        cache.read_state().last_reconciled_hash.as_deref(),
        Some(COMPOSED_HASH)
    );
    assert_eq!(
        outcome_with_pending(&unchanged_rows, pending(&root, &cache)),
        RefreshOutcome::Unchanged
    );

    drop(command);
    database.drop_now().await;
}
