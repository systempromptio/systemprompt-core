//! Debug-format contracts of the four `AppContext` planes. Planes are
//! assembled directly (the `from_parts` embedder path) against the test
//! database; tests skip when no database is configured.

use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_governance::authz::{AllowAllHook, NullAuditSink};
use systemprompt_manifest::profile::PathsConfig;
use systemprompt_models::RouteClassifier;
use systemprompt_runtime::{
    AppContext, ConfigPlane, ContextPlanes, DataPlane, Plugins, Subsystems,
};
use systemprompt_test_fixtures::{fixture_system_admin, unique_instance};

fn tmp_paths() -> PathsConfig {
    PathsConfig {
        root: None,
        system: "/tmp".to_string(),
        services: "/tmp".to_string(),
        bin: "/tmp".to_string(),
        web_path: Some("/tmp".to_string()),
        storage: Some("/tmp".to_string()),
        geoip_database: None,
        shipped_files: None,
        data: None,
    }
}

#[tokio::test]
async fn plane_debug_impls_flag_optional_members() {
    let test_instance = unique_instance();
    let url = systemprompt_test_fixtures::test_database_url();
    let pool = systemprompt_test_fixtures::test_db_pool().await;

    let data = DataPlane {
        database: Arc::clone(&pool),
        fingerprint_repo: None,
        user_service: None,
        oauth_repositories: Arc::new(systemprompt_oauth::repository::OAuthRepositories::new(
            &pool,
            Arc::from("pepper"),
        )),
        user_repository: Arc::new(systemprompt_users::UserRepository::new(&pool)),
        service_repository: Arc::new(systemprompt_database::ServiceRepository::new(
            &pool,
            test_instance.clone(),
        )),
        coordination: Arc::new(systemprompt_database::CoordinationRepositories::new(
            &pool,
            test_instance.clone(),
        )),
    };
    let dbg = format!("{data:?}");
    assert!(dbg.contains("DataPlane"), "got: {dbg}");
    assert!(dbg.contains("fingerprint_repo: false"), "got: {dbg}");
    assert!(dbg.contains("user_service: false"), "got: {dbg}");

    let app_paths = Arc::new(
        AppPaths::from_profile(
            &tmp_paths(),
            systemprompt_manifest::PathResolution::Canonicalize,
            None,
        )
        .expect("app paths"),
    );
    let cfg = ConfigPlane {
        config: Arc::new(systemprompt_test_fixtures::app_context::fixture_config(
            &url,
        )),
        files_config: Arc::new(systemprompt_files::FilesConfig::new(
            app_paths.storage().root().to_path_buf(),
            systemprompt_files::FilesConfigYaml::default(),
        )),
        app_paths,
        route_classifier: Arc::new(RouteClassifier::new(None)),
        build: systemprompt_models::BuildIdentity::core(),
        boot: systemprompt_test_fixtures::ensure_test_bootstrap()
            .boot
            .clone(),
    };
    let dbg = format!("{cfg:?}");
    assert!(dbg.contains("ConfigPlane"), "got: {dbg}");

    let plugins = Plugins {
        subject_providers: Default::default(),
        subjects: Default::default(),
    };
    let dbg = format!("{plugins:?}");
    assert!(dbg.contains("Plugins"), "got: {dbg}");
    assert!(dbg.contains("subject_providers"), "got: {dbg}");

    let authz_hook: systemprompt_governance::authz::SharedAuthzHook =
        Arc::new(AllowAllHook::new(Arc::new(NullAuditSink)));
    let file_storage =
        systemprompt_storage::build_file_storage(systemprompt_storage::FileStorageBackend::Local {
            root: std::env::temp_dir(),
        });
    let event_router = systemprompt_events::EventRouter::local_only(Arc::new(
        systemprompt_events::EventBroadcasters::new(),
    ));
    let subsystems = Subsystems {
        system_admin: Arc::new(fixture_system_admin("planeadmin")),
        logging: systemprompt_test_fixtures::fixture_logging_runtime(),
        authz_hook: Arc::clone(&authz_hook),
        signing_keys: systemprompt_test_fixtures::test_signing_keys(),
        token_authority: Arc::new(systemprompt_security::DeploymentTokenAuthority::new(
            systemprompt_test_fixtures::test_signing_keys(),
            "planes-issuer".to_owned(),
        )),
        governance: systemprompt_test_fixtures::default_governance_watch(),
        services: systemprompt_test_fixtures::fixture_services_state(),
        cache_epochs: systemprompt_traits::CacheEpochs::new(),
        schema_install: Arc::new(systemprompt_runtime::SchemaState::from(
            systemprompt_database::SchemaInstallReport::default(),
        )),
        event_bridge: systemprompt_test_fixtures::fixture_event_bridge(&pool),
        event_router,
        geoip_reader: None,
        file_storage: Arc::clone(&file_storage),
        shutdown: Default::default(),
        background_tasks: Default::default(),
        spawner: systemprompt_test_fixtures::test_spawner(),
        late_binders: Default::default(),
    };
    let dbg = format!("{subsystems:?}");
    assert!(dbg.contains("Subsystems"), "got: {dbg}");
    assert!(dbg.contains("system_admin: \"planeadmin\""), "got: {dbg}");
    assert!(dbg.contains("event_bridge: NotStarted"), "got: {dbg}");
    assert!(dbg.contains("outbox: None"), "got: {dbg}");
    assert!(dbg.contains("geoip_reader: false"), "got: {dbg}");

    let resolved = systemprompt_test_fixtures::test_app_context(&url).await;
    AppContext::from_parts(
        ContextPlanes {
            data,
            cfg,
            plugins,
            subsystems,
        },
        Arc::clone(resolved.kernel()),
        Arc::clone(resolved.admitted()),
    )
    .expect("context over the resolved core kernel");
}
