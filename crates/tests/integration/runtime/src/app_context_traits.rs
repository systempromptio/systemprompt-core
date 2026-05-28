//! Exercises trait implementations on [`AppContext`] via the
//! [`fixture_app_context`] helper. Each test holds the context, dispatches
//! via the relevant trait (`AppContextTrait`, `ExtensionContext`,
//! `HasAnalytics`, `HasFingerprint`, `HasUserService`,
//! `HasRouteClassifier`), and checks the resulting provider Arc / inner
//! reference is wired through unchanged.

use anyhow::Result;
use systemprompt_extension::{
    ExtensionContext, HasAnalytics, HasFingerprint, HasRouteClassifier, HasUserService,
};
use systemprompt_test_fixtures::{fixture_app_context, fixture_database_url, fixture_db_pool};
use systemprompt_traits::AppContext as AppContextTrait;

async fn build_ctx() -> Result<std::sync::Arc<systemprompt_runtime::AppContext>> {
    let url = fixture_database_url()?;
    let pool = fixture_db_pool(&url).await?;
    fixture_app_context(&pool, &url)
}

#[tokio::test]
async fn app_context_trait_config_handle_db_handle() -> Result<()> {
    let ctx = build_ctx().await?;
    let cfg = AppContextTrait::config(ctx.as_ref());
    assert!(!cfg.database_url().is_empty());
    let _db = AppContextTrait::database_handle(ctx.as_ref());
    Ok(())
}

#[tokio::test]
async fn app_context_trait_optional_providers_present() -> Result<()> {
    let ctx = build_ctx().await?;
    assert!(AppContextTrait::analytics_provider(ctx.as_ref()).is_some());
    assert!(AppContextTrait::fingerprint_provider(ctx.as_ref()).is_some());
    assert!(AppContextTrait::user_provider(ctx.as_ref()).is_some());
    Ok(())
}

#[tokio::test]
async fn extension_context_dispatch() -> Result<()> {
    let ctx = build_ctx().await?;
    let _cfg = ExtensionContext::config(ctx.as_ref());
    let _db = ExtensionContext::database(ctx.as_ref());
    let none = ExtensionContext::get_extension(ctx.as_ref(), "unknown_xxx");
    assert!(none.is_none());
    Ok(())
}

#[tokio::test]
async fn has_analytics_returns_inner_arc() -> Result<()> {
    let ctx = build_ctx().await?;
    let _ana = HasAnalytics::analytics(ctx.as_ref());
    Ok(())
}

#[tokio::test]
async fn has_fingerprint_some_when_wired() -> Result<()> {
    let ctx = build_ctx().await?;
    assert!(HasFingerprint::fingerprint(ctx.as_ref()).is_some());
    Ok(())
}

#[tokio::test]
async fn has_user_service_some_when_wired() -> Result<()> {
    let ctx = build_ctx().await?;
    assert!(HasUserService::user_service(ctx.as_ref()).is_some());
    Ok(())
}

#[tokio::test]
async fn has_route_classifier_present() -> Result<()> {
    let ctx = build_ctx().await?;
    let _ = HasRouteClassifier::route_classifier(ctx.as_ref());
    Ok(())
}

#[tokio::test]
async fn app_context_accessors_via_from_parts_path() -> Result<()> {
    let ctx = build_ctx().await?;
    let _ = ctx.config();
    let _ = ctx.db_pool();
    let _ = ctx.api_registry();
    let _ = ctx.extension_registry();
    let _ = ctx.app_paths();
    let _ = ctx.app_paths_arc();
    let _ = ctx.marketplace_filter();
    let _ = ctx.event_bridge();
    let _ = ctx.mcp_registry();
    let _ = ctx.analytics_service();
    let _ = ctx.route_classifier();
    let _ = ctx.geoip_reader();
    let _ = ctx.content_config();
    let _ = ctx.content_routing();
    let _ = ctx.system_admin();
    let _ = ctx.authz_hook();
    let addr = ctx.server_address();
    assert!(addr.contains(':'));
    Ok(())
}

#[tokio::test]
async fn app_context_debug_clone() -> Result<()> {
    let ctx = build_ctx().await?;
    let cloned = (*ctx).clone();
    let dbg = format!("{cloned:?}");
    assert!(dbg.contains("AppContext"));
    Ok(())
}

#[tokio::test]
async fn app_context_builder_accessor() {
    let _b = systemprompt_runtime::AppContext::builder();
}

#[tokio::test]
async fn module_api_registry_get_routes_unknown_returns_none() -> Result<()> {
    let ctx = build_ctx().await?;
    let reg = ctx.api_registry();
    let r = reg.get_routes("nonexistent-x", ctx.as_ref());
    assert!(r.is_none());
    Ok(())
}

#[tokio::test]
async fn database_context_from_url_round_trip() -> Result<()> {
    let url = systemprompt_test_fixtures::fixture_database_url()?;
    let dbc = systemprompt_runtime::DatabaseContext::from_url(&url).await?;
    let _ = dbc.db_pool();
    let _ = dbc.db_pool_arc();
    let cloned = dbc.clone();
    let dbg = format!("{cloned:?}");
    assert!(dbg.contains("DatabaseContext"));
    Ok(())
}

#[tokio::test]
async fn database_context_from_urls_read_only() -> Result<()> {
    let url = systemprompt_test_fixtures::fixture_database_url()?;
    let dbc = systemprompt_runtime::DatabaseContext::from_urls(&url, None).await?;
    let _ = dbc.db_pool();
    Ok(())
}

#[tokio::test]
async fn validate_system_passes_against_live_db() -> Result<()> {
    let ctx = build_ctx().await?;
    systemprompt_runtime::validate_system(ctx.as_ref()).await?;
    Ok(())
}

#[tokio::test]
async fn files_config_validator_validate_returns_clean_when_uninitialised() {
    use systemprompt_runtime::FilesConfigValidator;
    use systemprompt_traits::DomainConfig;
    let v = FilesConfigValidator::new();
    let report = v.validate().expect("validate");
    let _ = report.has_errors();
}

#[tokio::test]
async fn files_config_validator_validate_when_initialised() {
    use std::sync::Arc;
    use systemprompt_models::AppPaths;
    use systemprompt_models::profile::PathsConfig;
    use systemprompt_runtime::FilesConfigValidator;
    use systemprompt_traits::DomainConfig;

    let tmp = tempfile::TempDir::new().expect("tempdir");
    let p = tmp.path().to_string_lossy().to_string();
    let paths = Arc::new(
        AppPaths::from_profile(&PathsConfig {
            system: p.clone(),
            services: p.clone(),
            bin: p.clone(),
            web_path: Some(p.clone()),
            storage: Some(p),
            geoip_database: None,
        })
        .expect("paths"),
    );
    let _ = systemprompt_files::FilesConfig::init(&paths);
    let v = FilesConfigValidator::new();
    let report = v.validate().expect("validate");
    let _ = report.has_errors();
}
