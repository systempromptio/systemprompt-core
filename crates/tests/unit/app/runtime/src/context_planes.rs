//! Debug-format contracts of the four `AppContext` planes and the
//! `content_routing` accessor's Some arm. Planes are assembled directly (the
//! `from_parts` embedder path) against the test database; tests skip when no
//! database is configured.

use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_governance::authz::{AllowAllHook, NullAuditSink};
use systemprompt_manifest::profile::PathsConfig;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::{ContentConfigRaw, RouteClassifier};
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

fn content_config() -> Arc<ContentConfigRaw> {
    Arc::new(serde_yaml::from_str("{}").expect("empty content config"))
}

#[tokio::test]
async fn plane_debug_impls_flag_optional_members() {
    let test_instance = unique_instance();
    let url = systemprompt_test_fixtures::test_database_url();
    let pool = systemprompt_test_fixtures::test_db_pool().await;

    let session_usage: systemprompt_traits::DynSessionUsageCounters =
        Arc::new(systemprompt_users::SessionRepository::new(&pool));
    let data = DataPlane {
        database: Arc::clone(&pool),
        fingerprint_repo: None,
        user_service: None,
        a2a_repositories: Arc::new(systemprompt_agent::repository::A2ARepositories::new(
            &pool,
            systemprompt_agent::repository::A2aDependencies {
                session_usage,
                instance_id: test_instance.clone(),
                managed_skills: systemprompt_test_fixtures::not_managed_skills(),
                tool_executions: systemprompt_test_fixtures::tool_execution_ledger(
                    systemprompt_test_fixtures::ToolExecutionLedger::Exists,
                ),
            },
        )),
        content_repositories: Arc::new(systemprompt_content::repository::ContentRepositories::new(
            &pool,
        )),
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
        ai_repositories: Arc::new(systemprompt_ai::repository::AiRepositories::new(
            &pool,
            systemprompt_security::at_rest::AtRestCipher::from_secrets(
                &systemprompt_test_fixtures::ensure_test_secrets_bootstrap().get(),
            )
            .expect("at-rest cipher"),
        )),
        file_repository: Arc::new(systemprompt_files::FileRepository::new(&pool)),
        mcp_session_repository: Arc::new(systemprompt_mcp::repository::McpSessionRepository::new(
            &pool,
        )),
        managed_repository: Arc::new(systemprompt_marketplace::managed::ManagedRepository::new(
            &pool,
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
        content_config: Some(content_config()),
        route_classifier: Arc::new(RouteClassifier::new(None)),
        build: systemprompt_models::BuildIdentity::core(),
        boot: systemprompt_test_fixtures::ensure_test_bootstrap()
            .boot
            .clone(),
    };
    let dbg = format!("{cfg:?}");
    assert!(dbg.contains("ConfigPlane"), "got: {dbg}");
    assert!(dbg.contains("content_config: true"), "got: {dbg}");

    let plugins = Plugins {
        mcp_registry: RegistryService::new(
            systemprompt_test_fixtures::fixture_user_id(),
            systemprompt_test_fixtures::fixture_services_view(),
            systemprompt_test_fixtures::test_config(),
        ),
        marketplace_filter: Arc::new(AllowAllFilter),
        marketplace_cache: Arc::new(systemprompt_marketplace::MarketplaceCache::default()),
        subject_providers: Default::default(),
        subjects: Default::default(),
    };
    let dbg = format!("{plugins:?}");
    assert!(dbg.contains("Plugins"), "got: {dbg}");
    assert!(dbg.contains("marketplace_filter"), "got: {dbg}");

    let authz_hook: systemprompt_governance::authz::SharedAuthzHook =
        Arc::new(AllowAllHook::new(Arc::new(NullAuditSink)));
    let file_storage =
        systemprompt_storage::build_file_storage(systemprompt_storage::FileStorageBackend::Local {
            root: std::env::temp_dir(),
        });
    let documents =
        systemprompt_test_fixtures::fixture_documents_runtime(&pool, &file_storage, &authz_hook);
    let event_router = systemprompt_events::EventRouter::local_only(Arc::new(
        systemprompt_events::EventBroadcasters::new(),
    ));
    let subsystems = Subsystems {
        system_admin: Arc::new(fixture_system_admin("planeadmin")),
        logging: systemprompt_test_fixtures::fixture_logging_runtime(),
        authz_hook: Arc::clone(&authz_hook),
        signing_keys: systemprompt_test_fixtures::test_signing_keys(),
        governance: systemprompt_test_fixtures::default_governance_watch(),
        services: systemprompt_test_fixtures::fixture_services_state(),
        cache_epochs: systemprompt_traits::CacheEpochs::new(),
        ai_service: None,
        artifact_ingest: systemprompt_test_fixtures::fixture_artifact_ingest(&pool)
            .expect("artifact ingest"),
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
        publish_guard: Arc::new(tokio::sync::Mutex::new(
            systemprompt_marketplace::inventory::PublishGuard::default(),
        )),
        connector_tokens: Arc::default(),
        documents,
    };
    let dbg = format!("{subsystems:?}");
    assert!(dbg.contains("Subsystems"), "got: {dbg}");
    assert!(dbg.contains("system_admin: \"planeadmin\""), "got: {dbg}");
    assert!(dbg.contains("event_bridge: NotStarted"), "got: {dbg}");
    assert!(dbg.contains("outbox: None"), "got: {dbg}");
    assert!(dbg.contains("geoip_reader: false"), "got: {dbg}");

    let resolved = systemprompt_test_fixtures::test_app_context(&url).await;
    let ctx = AppContext::from_parts(
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
    assert!(
        ctx.content_config().is_some(),
        "content_config accessor must expose the raw config"
    );
}
