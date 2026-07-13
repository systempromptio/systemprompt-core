//! Debug-format contracts of the four `AppContext` planes and the
//! `content_routing` accessor's Some arm. Planes are assembled directly (the
//! `from_parts` embedder path) against the test database; tests skip when no
//! database is configured.

use std::sync::{Arc, OnceLock};

use systemprompt_analytics::AnalyticsService;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::profile::PathsConfig;
use systemprompt_models::{AppPaths, ContentConfigRaw, RouteClassifier};
use systemprompt_runtime::{
    AppContext, ConfigPlane, DataPlane, ModuleApiRegistry, Plugins, Subsystems,
};
use systemprompt_security::authz::{AllowAllHook, NullAuditSink};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, fixture_system_admin};

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

macro_rules! pool_or_skip {
    () => {{
        let Ok(url) = fixture_database_url() else {
            return;
        };
        let Ok(pool) = fixture_db_pool(&url).await else {
            return;
        };
        (pool, url)
    }};
}

#[tokio::test]
async fn plane_debug_impls_flag_optional_members() {
    let (pool, url) = pool_or_skip!();

    let data = DataPlane {
        database: Arc::clone(&pool),
        analytics_service: Arc::new(
            AnalyticsService::new(&pool, None, None).expect("analytics service"),
        ),
        fingerprint_repo: None,
        user_service: None,
    };
    let dbg = format!("{data:?}");
    assert!(dbg.contains("DataPlane"), "got: {dbg}");
    assert!(dbg.contains("fingerprint_repo: false"), "got: {dbg}");
    assert!(dbg.contains("user_service: false"), "got: {dbg}");

    let cfg = ConfigPlane {
        config: Arc::new(systemprompt_test_fixtures::app_context::fixture_config(
            &url,
        )),
        app_paths: Arc::new(AppPaths::from_profile(&tmp_paths()).expect("app paths")),
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
    };
    let dbg = format!("{plugins:?}");
    assert!(dbg.contains("Plugins"), "got: {dbg}");
    assert!(dbg.contains("marketplace_filter"), "got: {dbg}");

    let subsystems = Subsystems {
        system_admin: Arc::new(fixture_system_admin("planeadmin")),
        authz_hook: Arc::new(AllowAllHook::new(Arc::new(NullAuditSink))),
        event_bridge: Arc::new(OnceLock::new()),
        geoip_reader: None,
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
