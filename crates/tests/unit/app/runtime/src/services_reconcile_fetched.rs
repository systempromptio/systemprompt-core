//! Boot-time authz projection of a fetched services composition.
//!
//! Every failure here is a refused boot: an instance that swapped in a new
//! composition and could not project it would serve the old grants against
//! the new catalog, so each arm asserts both the error and that the
//! composition was *not* recorded as reconciled.

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::Utc;
use systemprompt_config::ProfileBootstrap;
use systemprompt_database::DbPool;
use systemprompt_loader::bundle::{BundleCache, cache_root};
use systemprompt_loader::{ActiveServicesRoot, ServicesBootstrap, ServicesProvenance};
use systemprompt_models::Profile;
use systemprompt_models::profile::ServicesSource;
use systemprompt_models::services::ServicesConfig;
use systemprompt_models::services::bundle::{
    BundleOwnership, BundleSourceInfo, BundleSourceState, ServicesBundleManifest,
    ServicesBundleState, SignedBundleManifest,
};
use systemprompt_runtime::RuntimeError;
use systemprompt_runtime::services_reconcile::reconcile_fetched_services;
use systemprompt_test_fixtures::{
    DisposableDb, closed_db_pool, ensure_test_bootstrap, fixture_db_pool,
};
use tempfile::TempDir;

const SOURCE: &str = "astound";
const CONTENT_HASH: &str = "content-hash-1";
const COMPOSED: &str = "composed-hash-1";

struct Fixture {
    _tmp: TempDir,
    profile: Profile,
    cache: BundleCache,
}

fn fixture(with_source: bool) -> Fixture {
    ensure_test_bootstrap();
    let tmp = TempDir::new().expect("tempdir");
    let mut profile = ProfileBootstrap::get()
        .expect("bootstrapped profile")
        .clone();
    profile.services.cache_dir = Some(tmp.path().to_string_lossy().into_owned());
    profile.services.sources = if with_source {
        vec![ServicesSource {
            name: SOURCE.to_owned(),
            https: None,
            oci: None,
        }]
    } else {
        Vec::new()
    };
    let cache = BundleCache::new(cache_root(&profile));
    Fixture {
        _tmp: tmp,
        profile,
        cache,
    }
}

fn services() -> &'static ServicesConfig {
    ServicesBootstrap::get().expect("bootstrapped services config")
}

fn fetched_root(path: &std::path::Path) -> ActiveServicesRoot {
    ActiveServicesRoot {
        path: path.to_path_buf(),
        provenance: ServicesProvenance::Fetched {
            composed_hash: COMPOSED.to_owned(),
            versions: BTreeMap::new(),
        },
    }
}

fn state_with_source() -> ServicesBundleState {
    let mut sources = BTreeMap::new();
    sources.insert(
        SOURCE.to_owned(),
        BundleSourceState {
            digest: "sha256:aaa".to_owned(),
            version: "1.0.0".to_owned(),
            content_hash: CONTENT_HASH.to_owned(),
            fetched_at: Utc::now(),
        },
    );
    ServicesBundleState {
        composed_hash: COMPOSED.to_owned(),
        last_reconciled_hash: None,
        sources,
    }
}

fn write_manifest(cache: &BundleCache, owns: BundleOwnership) {
    let dir = cache.bundle_dir(SOURCE, CONTENT_HASH);
    std::fs::create_dir_all(&dir).expect("bundle dir");
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
            owns,
        },
        signature: None,
    };
    std::fs::write(
        dir.join("bundle.json"),
        serde_json::to_vec_pretty(&signed).expect("manifest serialises"),
    )
    .expect("write manifest");
}

fn message(err: &RuntimeError) -> String {
    match err {
        RuntimeError::Internal(m) => m.clone(),
        other => panic!("expected an internal error, got {other:?}"),
    }
}

#[tokio::test]
async fn a_baked_tree_is_reconciled_without_touching_the_database() {
    let f = fixture(false);
    let db: DbPool = closed_db_pool().await;

    reconcile_fetched_services(
        &f.profile,
        &ActiveServicesRoot {
            path: PathBuf::from(&f.profile.paths.services),
            provenance: ServicesProvenance::Bundled,
        },
        services(),
        &db,
    )
    .await
    .expect("a baked tree needs no projection, so an unreachable database is irrelevant");

    assert_eq!(
        f.cache.read_state().last_reconciled_hash,
        None,
        "nothing was projected, so nothing may be recorded"
    );
}

#[tokio::test]
async fn a_source_with_no_cached_fetch_state_refuses_the_boot() {
    let f = fixture(true);
    let db: DbPool = closed_db_pool().await;

    let err =
        reconcile_fetched_services(&f.profile, &fetched_root(f.cache.root()), services(), &db)
            .await
            .expect_err("a composed source with no cached fetch state cannot be projected");

    let text = message(&err);
    assert!(
        text.contains(SOURCE) && text.contains("no cached fetch state"),
        "the error must name the source that is missing: {text}"
    );
    assert_eq!(f.cache.read_state().last_reconciled_hash, None);
}

#[tokio::test]
async fn an_unreadable_cached_manifest_refuses_the_boot() {
    let f = fixture(true);
    f.cache
        .write_state(&state_with_source())
        .expect("seed cache state");
    let db: DbPool = closed_db_pool().await;

    let err =
        reconcile_fetched_services(&f.profile, &fetched_root(f.cache.root()), services(), &db)
            .await
            .expect_err("a cached manifest that is not on disk cannot be projected");

    let text = message(&err);
    assert!(
        text.contains(SOURCE) && text.contains("manifest"),
        "the error must name the unreadable manifest: {text}"
    );
    assert_eq!(f.cache.read_state().last_reconciled_hash, None);
}

#[tokio::test]
async fn a_failed_projection_refuses_the_boot_and_records_nothing() {
    let f = fixture(true);
    f.cache
        .write_state(&state_with_source())
        .expect("seed cache state");
    write_manifest(&f.cache, BundleOwnership::default());
    let db: DbPool = closed_db_pool().await;

    let err =
        reconcile_fetched_services(&f.profile, &fetched_root(f.cache.root()), services(), &db)
            .await
            .expect_err("a projection that cannot reach the database must refuse the boot");

    assert!(
        message(&err).contains("services authz reconcile"),
        "the reconcile failure must be reported as such: {}",
        message(&err)
    );
    assert_eq!(
        f.cache.read_state().last_reconciled_hash,
        None,
        "an unprojected composition must never be recorded as reconciled"
    );
}

#[tokio::test]
async fn a_projected_composition_is_recorded_and_not_projected_again() {
    let boot = ensure_test_bootstrap();
    let Ok(disposable) = DisposableDb::installed("services_reconcile").await else {
        return;
    };
    let f = fixture(true);
    f.cache
        .write_state(&state_with_source())
        .expect("seed cache state");
    write_manifest(&f.cache, BundleOwnership::default());
    let db = fixture_db_pool(disposable.url())
        .await
        .expect("disposable pool");
    let root = ActiveServicesRoot {
        path: boot.services_path.clone(),
        provenance: ServicesProvenance::Fetched {
            composed_hash: COMPOSED.to_owned(),
            versions: BTreeMap::new(),
        },
    };

    reconcile_fetched_services(&f.profile, &root, services(), &db)
        .await
        .expect("a composition over a migrated database projects");

    assert_eq!(
        f.cache.read_state().last_reconciled_hash.as_deref(),
        Some(COMPOSED),
        "a projected composition must be recorded so a restart does not re-project it"
    );

    let closed = closed_db_pool().await;
    reconcile_fetched_services(&f.profile, &root, services(), &closed)
        .await
        .expect("the recorded composition makes the step a no-op on restart");

    disposable.drop_now().await;
}
