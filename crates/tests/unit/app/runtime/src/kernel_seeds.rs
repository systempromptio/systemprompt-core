//! Each kernel seed is what a provide fn reads: withheld, resolving a member
//! that reads it fails naming the seed's type; supplied, the same member
//! resolves. The admitted views the kernel inserts on resolve are present to
//! every provide fn and absent from a kernel that never resolved. The fixture
//! seed set a test resolves members over names the same open and sealed host
//! types as the API host's, apart from the seeds only the API composes.

use std::collections::BTreeSet;
use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_database::DbPool;
use systemprompt_events::{EventBroadcasters, EventRouter};
use systemprompt_extension::prelude::Dependencies;
use systemprompt_extension::{
    Admitted, AdmittedAssets, AdmittedConfigs, AdmittedMigrations, AdmittedProjections,
    AdmittedPurge, AdmittedSchedule, MissingDependency, Resolved,
};
use systemprompt_files::FilesConfig;
use systemprompt_files::documents::DocumentsHandlesSlot;
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::services::ServicesView;
use systemprompt_oauth::repository::OAuthRepositories;
use systemprompt_storage::LocalFileStorage;
use systemprompt_test_fixtures::{
    ComposedParts, FixtureSeedParts, Probe, ensure_test_bootstrap, fixture_documents_admitted,
    fixture_kernel, fixture_kernel_seed, lazy_pg_pool, probe_admitted, resolved_kernel,
    test_app_context, test_database_url,
};
use systemprompt_traits::{BackgroundTasks, FileStorage};

#[derive(Clone)]
struct SeedRead;

fn reader_of<T: Clone + Send + Sync + 'static>() -> Admitted {
    probe_admitted(ComposedParts {
        probes: vec![Probe::new("seed-reader", |c| {
            c.provide::<SeedRead, _>(|r: &Resolved<'_>| {
                r.get::<T>()?;
                Ok(SeedRead)
            });
        })],
        ..ComposedParts::default()
    })
}

fn offline_pool() -> DbPool {
    Arc::new(systemprompt_database::Database::from_pools(
        lazy_pg_pool(),
        None,
    ))
}

async fn kernel_state() -> Dependencies {
    ensure_test_bootstrap();
    test_app_context(&test_database_url())
        .await
        .kernel()
        .state()
        .clone()
}

async fn assert_seed<T: Clone + Send + Sync + 'static>() {
    let value = kernel_state()
        .await
        .find::<T>()
        .cloned()
        .unwrap_or_else(|| panic!("the kernel state seeds {}", std::any::type_name::<T>()));
    let admitted = reader_of::<T>();
    let pool = offline_pool();

    let withheld = resolved_kernel(&pool, &admitted, Dependencies::new())
        .err()
        .unwrap_or_else(|| {
            panic!(
                "{} withheld must refuse the reader",
                std::any::type_name::<T>()
            )
        });
    let mut rendered = withheld.to_string();
    let mut source = std::error::Error::source(&withheld);
    while let Some(cause) = source {
        rendered.push_str(": ");
        rendered.push_str(&cause.to_string());
        source = cause.source();
    }
    assert!(
        rendered.contains(std::any::type_name::<T>()),
        "the refusal names the withheld seed: {rendered}"
    );

    let kernel = resolved_kernel(&pool, &admitted, Dependencies::new().with(value))
        .unwrap_or_else(|e| panic!("{} supplied resolves: {e:#}", std::any::type_name::<T>()));
    assert!(kernel.resolved().get::<SeedRead>().is_ok());
}

fn assert_admitted_view<T: Clone + Send + Sync + 'static>() {
    let unresolved = fixture_kernel(&offline_pool(), Dependencies::new());
    assert_eq!(
        unresolved.resolved().get::<T>().err(),
        Some(MissingDependency::of::<T>()),
        "a kernel that never resolved holds no {}",
        std::any::type_name::<T>()
    );
    let kernel = resolved_kernel(&offline_pool(), &reader_of::<T>(), Dependencies::new())
        .unwrap_or_else(|e| {
            panic!(
                "{} is inserted on resolve: {e:#}",
                std::any::type_name::<T>()
            )
        });
    assert!(kernel.resolved().get::<SeedRead>().is_ok());
}

#[tokio::test]
async fn the_database_pool_seed() {
    assert_seed::<DbPool>().await;
}

#[tokio::test]
async fn the_app_paths_seed() {
    assert_seed::<Arc<AppPaths>>().await;
}

#[tokio::test]
async fn the_files_config_seed() {
    assert_seed::<Arc<FilesConfig>>().await;
}

#[tokio::test]
async fn the_instance_id_seed() {
    assert_seed::<InstanceId>().await;
}

#[tokio::test]
async fn the_services_view_seed() {
    assert_seed::<ServicesView>().await;
}

#[tokio::test]
async fn the_documents_handles_seed() {
    assert_seed::<DocumentsHandlesSlot>().await;
}

#[tokio::test]
async fn the_oauth_repositories_seed() {
    assert_seed::<Arc<OAuthRepositories>>().await;
}

#[test]
fn the_admitted_purge_view() {
    assert_admitted_view::<AdmittedPurge>();
}

#[test]
fn the_admitted_schedule_view() {
    assert_admitted_view::<AdmittedSchedule>();
}

#[test]
fn the_admitted_migrations_view() {
    assert_admitted_view::<AdmittedMigrations>();
}

#[test]
fn the_admitted_configs_view() {
    assert_admitted_view::<AdmittedConfigs>();
}

#[test]
fn the_admitted_assets_view() {
    assert_admitted_view::<AdmittedAssets>();
}

#[test]
fn the_admitted_projections_view() {
    assert_admitted_view::<AdmittedProjections>();
}

const API_ONLY_SEEDS: &[&str] = &[
    "systemprompt_security::keys::signing_keys::SigningKeys",
    "alloc::sync::Arc<systemprompt_loader::subprocess::spawn::Spawner>",
    "alloc::sync::Arc<dyn systemprompt_config::bootstrap::secrets::child::ChildSecretEnv>",
    "systemprompt_security::keys::verification::TokenVerificationKeys",
    "alloc::sync::Arc<systemprompt_oauth::repository::OAuthRepositories>",
    "alloc::sync::Arc<dyn systemprompt_models::auth::token_authority::TokenAuthority>",
    "systemprompt_models::late_bound::LateRead<dyn systemprompt_traits::messaging::MessagingDispatcher>",
    "systemprompt_models::late_bound::LateRead<dyn systemprompt_models::a2a::dispatch::AgentDispatch>",
    "alloc::sync::Arc<maxminddb::reader::Reader<alloc::vec::Vec<u8>>>",
    "alloc::sync::Arc<systemprompt_models::content_config::ContentConfigRaw>",
];

fn host_seed_names(state: &Dependencies) -> (BTreeSet<&'static str>, BTreeSet<&'static str>) {
    let host: BTreeSet<&'static str> = state.host_types().map(|(_, name)| name).collect();
    let sealed: BTreeSet<&'static str> = state
        .sealed_types()
        .map(|(_, name)| name)
        .filter(|name| host.contains(name))
        .collect();
    let open = host.difference(&sealed).copied().collect();
    (open, sealed)
}

#[tokio::test]
async fn the_fixture_seed_set_names_the_hosts_open_and_sealed_types() {
    let host = kernel_state().await;
    let pool = offline_pool();
    let storage = tempfile::tempdir().expect("storage dir");
    let file_storage: Arc<dyn FileStorage> =
        Arc::new(LocalFileStorage::new(storage.path().to_path_buf()));
    let (documents, _binder) = DocumentsHandlesSlot::unbound();
    let seed = fixture_kernel_seed(
        &pool,
        FixtureSeedParts {
            file_storage,
            background_tasks: BackgroundTasks::new(),
            event_router: EventRouter::local_only(Arc::new(EventBroadcasters::new())),
            documents,
        },
    );
    let fixture = resolved_kernel(&pool, &fixture_documents_admitted(), seed)
        .unwrap_or_else(|e| panic!("the document members resolve over the fixture seeds: {e:#}"))
        .state()
        .clone();

    let (host_open, host_sealed) = host_seed_names(&host);
    for name in API_ONLY_SEEDS {
        assert!(
            host_open.contains(name) || host_sealed.contains(name),
            "the API host seeds or reserves {name}"
        );
    }
    let (fixture_open, fixture_sealed) = host_seed_names(&fixture);
    let host_open: BTreeSet<_> = host_open
        .into_iter()
        .filter(|name| !API_ONLY_SEEDS.contains(name))
        .collect();
    let host_sealed: BTreeSet<_> = host_sealed
        .into_iter()
        .filter(|name| !API_ONLY_SEEDS.contains(name))
        .collect();
    for (kind, fixture, host) in [
        ("open", &fixture_open, &host_open),
        ("sealed", &fixture_sealed, &host_sealed),
    ] {
        let only_fixture: Vec<_> = fixture.difference(host).collect();
        let only_host: Vec<_> = host.difference(fixture).collect();
        assert!(
            only_fixture.is_empty() && only_host.is_empty(),
            "{kind} host seeds differ: only the fixture seeds {only_fixture:?}; only the host seeds {only_host:?}"
        );
    }
}
