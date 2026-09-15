//! Debug-format contracts of the four `AppContext` planes and the
//! `content_routing` accessor's Some arm. Planes are assembled directly (the
//! `from_parts` embedder path) against the test database; tests skip when no
//! database is configured.

use std::sync::{Arc, OnceLock};

use systemprompt_analytics::AnalyticsService;
use systemprompt_config::paths::AppPaths;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::profile::PathsConfig;
use systemprompt_models::{ContentConfigRaw, RouteClassifier};
use systemprompt_runtime::{
    AppContext, ConfigPlane, DataPlane, ModuleApiRegistry, Plugins, Subsystems,
};
use systemprompt_security::authz::{AllowAllHook, NullAuditSink};
use systemprompt_test_fixtures::fixture_system_admin;

fn tmp_paths() -> PathsConfig {
    PathsConfig {
        system: "/tmp".to_string(),
        services: "/tmp".to_string(),
        bin: "/tmp".to_string(),
        web_path: Some("/tmp".to_string()),
        storage: Some("/tmp".to_string()),
        geoip_database: None,
    }
}

fn content_config() -> Arc<ContentConfigRaw> {
    Arc::new(serde_yaml::from_str("{}").expect("empty content config"))
}

#[tokio::test]
async fn plane_debug_impls_flag_optional_members() {
    let (pool, url) = systemprompt_test_fixtures::db_pool_or_skip!();

    let analytics_service = Arc::new(AnalyticsService::new(
        None,
        None,
        &systemprompt_test_fixtures::fixture_analytics_repositories(&pool).expect("repositories"),
    ));
    let session_usage: systemprompt_traits::DynSessionUsageCounters =
        analytics_service.session_repo().owner();
    let sqlx_pool = pool.pool_arc().expect("SQLx pool").as_ref().clone();
    let data = DataPlane {
        database: Arc::clone(&pool),
        analytics_service,
        fingerprint_repo: None,
        user_service: None,
        a2a_repositories: Arc::new(
            systemprompt_agent::repository::A2ARepositories::new(
                &pool,
                systemprompt_agent::repository::A2aDependencies {
                    session_usage,
                    instance_id: systemprompt_identifiers::InstanceId::new("test-instance"),
                    managed_skills: systemprompt_test_fixtures::not_managed_skills(),
                    tool_executions: systemprompt_test_fixtures::tool_execution_ledger(
                        systemprompt_test_fixtures::ToolExecutionLedger::Exists,
                    ),
                },
            )
            .expect("a2a repositories"),
        ),
        content_repositories: Arc::new(
            systemprompt_content::repository::ContentRepositories::new(&pool)
                .expect("content repositories"),
        ),
        oauth_repositories: Arc::new(
            systemprompt_oauth::repository::OAuthRepositories::new(&pool)
                .expect("oauth repositories"),
        ),
        user_repository: Arc::new(
            systemprompt_users::UserRepository::new(&pool).expect("user repository"),
        ),
        service_repository: Arc::new(
            systemprompt_database::ServiceRepository::new(
                &pool,
                systemprompt_identifiers::InstanceId::new("test-instance"),
            )
            .expect("service repository"),
        ),
        ai_repositories: Arc::new(
            systemprompt_ai::repository::AiRepositories::new(&pool).expect("ai repositories"),
        ),
        analytics_repositories: Arc::new(
            systemprompt_test_fixtures::fixture_analytics_repositories(&pool)
                .expect("analytics repositories"),
        ),
        file_repository: Arc::new(
            systemprompt_files::FileRepository::new(&pool).expect("file repository"),
        ),
        mcp_session_repository: Arc::new(
            systemprompt_mcp::repository::McpSessionRepository::new(&pool)
                .expect("mcp session repository"),
        ),
        feedback_snapshots_repository: Arc::new(
            systemprompt_analytics::snapshots::FeedbackSnapshotsRepository::new(
                sqlx_pool.clone(),
                systemprompt_analytics::feedback::FeedbackFactsRepository::new(sqlx_pool.clone()),
            ),
        ),
        feedback_facts_repository: Arc::new(
            systemprompt_analytics::feedback::FeedbackFactsRepository::new(sqlx_pool.clone()),
        ),
        managed_repository: Arc::new(
            systemprompt_marketplace::managed::ManagedRepository::new(&pool)
                .expect("managed repository"),
        ),
        evaluation_repositories: Arc::new(
            systemprompt_test_fixtures::fixture_evaluation_repositories(&pool)
                .expect("evaluation repositories"),
        ),
    };
    let dbg = format!("{data:?}");
    assert!(dbg.contains("DataPlane"), "got: {dbg}");
    assert!(dbg.contains("fingerprint_repo: false"), "got: {dbg}");
    assert!(dbg.contains("user_service: false"), "got: {dbg}");

    let cfg = ConfigPlane {
        config: Arc::new(systemprompt_test_fixtures::app_context::fixture_config(
            &url,
        )),
        app_paths: Arc::new(
            AppPaths::from_profile(
                &tmp_paths(),
                systemprompt_models::PathResolution::Canonicalize,
                None,
            )
            .expect("app paths"),
        ),
        content_config: Some(content_config()),
        route_classifier: Arc::new(RouteClassifier::new(None)),
    };
    let dbg = format!("{cfg:?}");
    assert!(dbg.contains("ConfigPlane"), "got: {dbg}");
    assert!(dbg.contains("content_config: true"), "got: {dbg}");

    let plugins = Plugins {
        extension_registry: Arc::new(ExtensionRegistry::new()),
        api_registry: Arc::new(ModuleApiRegistry::new()),
        mcp_registry: RegistryService::new(systemprompt_test_fixtures::fixture_user_id()),
        marketplace_filter: Arc::new(AllowAllFilter),
        marketplace_cache: Arc::new(systemprompt_marketplace::MarketplaceCache::default()),
    };
    let dbg = format!("{plugins:?}");
    assert!(dbg.contains("Plugins"), "got: {dbg}");
    assert!(dbg.contains("marketplace_filter"), "got: {dbg}");

    let subsystems = Subsystems {
        system_admin: Arc::new(fixture_system_admin("planeadmin")),
        authz_hook: Arc::new(AllowAllHook::new(Arc::new(NullAuditSink))),
        governance: systemprompt_test_fixtures::default_governance_engine(),
        schema_install: Arc::new(systemprompt_database::SchemaInstallReport::default()),
        event_bridge: Arc::new(OnceLock::new()),
        geoip_reader: None,
        file_storage: systemprompt_storage::build_file_storage(
            systemprompt_models::profile::StorageBackend::Local,
            &std::env::temp_dir(),
        ),
        shutdown: Default::default(),
        publish_guard: Arc::new(tokio::sync::Mutex::new(
            systemprompt_marketplace::inventory::PublishGuard::default(),
        )),
        snapshot_wakeup: Arc::new(systemprompt_runtime::reporting::SnapshotWakeup::default()),
    };
    let dbg = format!("{subsystems:?}");
    assert!(dbg.contains("Subsystems"), "got: {dbg}");
    assert!(dbg.contains("system_admin: \"planeadmin\""), "got: {dbg}");
    assert!(dbg.contains("event_bridge: false"), "got: {dbg}");
    assert!(dbg.contains("geoip_reader: false"), "got: {dbg}");

    let ctx = AppContext::from_parts(data, cfg, plugins, subsystems);
    let routing = ctx.content_routing();
    assert!(
        routing.is_some(),
        "content_routing must be Some when a content config is present"
    );
    assert!(
        ctx.content_config().is_some(),
        "content_config accessor must expose the raw config"
    );
}
