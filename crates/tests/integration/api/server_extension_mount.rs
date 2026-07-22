//! Extension router mounting through `setup_api_server`: nested and
//! root-merged base paths, the public/auth wrapping split, and the
//! frame-options override stamp.

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use axum::routing::get;
use systemprompt_analytics::{AnalyticsService, FingerprintRepository};
use systemprompt_api::services::server::setup_api_server;
use systemprompt_extension::{
    Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry, ExtensionRouter,
    FrameOptions,
};
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::profile::PathsConfig;
use systemprompt_models::{AppPaths, RouteClassifier};
use systemprompt_runtime::{
    AppContext, ConfigPlane, DataPlane, ModuleApiRegistry, Plugins, Subsystems,
};
use systemprompt_security::authz::{AllowAllHook, NullAuditSink};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_config, fixture_db_pool, fixture_system_admin, fixture_user_id,
};
use systemprompt_users::UserService;
use tower::ServiceExt;

struct NestedPublicExt;

impl Extension for NestedPublicExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "covmount",
            name: "Coverage Mount Extension",
            version: "0.0.1",
        }
    }

    fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        let mut ext = ExtensionRouter::public(
            Router::new().route("/ping", get(|| async { "ext-ok" })),
            "/covmount",
        );
        ext.frame_options = Some(FrameOptions::AllowAll);
        Some(ext)
    }
}

struct RootMergedExt;

impl Extension for RootMergedExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "covroot",
            name: "Coverage Root Extension",
            version: "0.0.1",
        }
    }

    fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        Some(ExtensionRouter::public(
            Router::new().route("/covroot-ping", get(|| async { "root-ok" })),
            "/",
        ))
    }
}

struct AuthedExt;

impl Extension for AuthedExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "covauth",
            name: "Coverage Authed Extension",
            version: "0.0.1",
        }
    }

    fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        Some(ExtensionRouter::new(
            Router::new().route("/secret", get(|| async { "authed" })),
            "/covauth",
        ))
    }
}

async fn app_with_extensions() -> anyhow::Result<Router> {
    let bootstrap = ensure_test_bootstrap();
    let pool = fixture_db_pool(&bootstrap.database_url).await?;

    let mut config = fixture_config(&bootstrap.database_url);
    config.cors_allowed_origins = vec!["http://127.0.0.1".to_owned()];

    let paths = PathsConfig {
        system: "/tmp".to_string(),
        services: "/tmp".to_string(),
        bin: "/tmp".to_string(),
        web_path: Some("/tmp".to_string()),
        storage: Some("/tmp".to_string()),
        geoip_database: None,
    };
    let app_paths = Arc::new(AppPaths::from_profile(&paths)?);

    let registry = ExtensionRegistry::discover_and_merge(vec![
        Arc::new(NestedPublicExt),
        Arc::new(RootMergedExt),
        Arc::new(AuthedExt),
    ])
    .map_err(|e| anyhow::anyhow!("registry: {e}"))?;

    let ctx = Arc::new(AppContext::from_parts(
        DataPlane {
            database: Arc::clone(&pool),
            analytics_service: Arc::new(AnalyticsService::new(&pool, None, None)?),
            fingerprint_repo: Some(Arc::new(FingerprintRepository::new(&pool)?)),
            user_service: Some(Arc::new(UserService::new(&pool)?)),
        },
        ConfigPlane {
            config: Arc::new(config),
            app_paths,
            content_config: None,
            route_classifier: Arc::new(RouteClassifier::new(None)),
        },
        Plugins {
            extension_registry: Arc::new(registry),
            api_registry: Arc::new(ModuleApiRegistry::new()),
            mcp_registry: RegistryService::new(fixture_user_id()),
            marketplace_filter: Arc::new(AllowAllFilter),
        },
        Subsystems {
            system_admin: Arc::new(fixture_system_admin("admin")),
            authz_hook: Arc::new(AllowAllHook::new(Arc::new(NullAuditSink))),
            event_bridge: Arc::new(OnceLock::new()),
            geoip_reader: None,
        },
    ));

    setup_api_server(&ctx, None).map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))
}

fn get_req(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request build")
}

#[tokio::test]
async fn extension_routes_mount_across_nested_root_and_authed_paths() -> anyhow::Result<()> {
    let app = app_with_extensions().await?;

    let nested = app.clone().oneshot(get_req("/covmount/ping")).await?;
    assert_eq!(nested.status().as_u16(), 200, "{}", nested.status());
    assert!(
        nested.headers().get("x-frame-options").is_none(),
        "AllowAll frame override must suppress X-Frame-Options, got {:?}",
        nested.headers().get("x-frame-options")
    );
    let body = http_body_util::BodyExt::collect(nested.into_body())
        .await?
        .to_bytes();
    assert_eq!(&body[..], b"ext-ok");

    let root = app.clone().oneshot(get_req("/covroot-ping")).await?;
    assert_eq!(root.status().as_u16(), 200, "{}", root.status());

    let authed = app.oneshot(get_req("/covauth/secret")).await?;
    assert_eq!(
        authed.status().as_u16(),
        401,
        "auth-required extension route must reject anonymous requests, got {}",
        authed.status()
    );
    Ok(())
}
