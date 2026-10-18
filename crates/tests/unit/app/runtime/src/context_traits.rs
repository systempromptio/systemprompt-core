//! `AppContext` seen through the trait objects extensions and providers hold:
//! the `systemprompt_traits::AppContext` provider accessors, the
//! `ExtensionContext` handed to extension routers, and the `Has*` capability
//! traits. Each view must answer from the same planes as the concrete
//! accessors, so a caller holding only a trait object sees the same config,
//! database, owner and services as one holding the concrete context.

use std::sync::Arc;

use systemprompt_extension::{
    ExtensionContext, HasAnalytics, HasFingerprint, HasRouteClassifier, HasUserService,
};
use systemprompt_identifiers::{InstanceId, LeaseKey};
use systemprompt_models::RouteType;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, test_app_context, test_database_url, test_db_pool,
};
use systemprompt_traits::AppContext as AppContextTrait;

async fn context() -> (Arc<AppContext>, String) {
    ensure_test_bootstrap();
    let url = test_database_url();
    let pool = test_db_pool().await;
    (test_app_context(&pool, &url), url)
}

#[tokio::test]
async fn the_provider_view_shares_the_concrete_config_and_database() {
    let (ctx, url) = context().await;
    let view: &dyn AppContextTrait = ctx.as_ref();

    let config = AppContextTrait::config(view);
    assert_eq!(config.database_url(), url);
    assert_eq!(config.api_port(), ctx.config().port);
    assert!(view.database_handle().is_connected());
    assert!(view.session_provider().is_some());
    assert!(view.analytics_provider().is_some());
    assert_eq!(
        view.user_provider().is_some(),
        HasUserService::user_service(ctx.as_ref()).is_some()
    );
    assert_eq!(
        view.fingerprint_provider().is_some(),
        HasFingerprint::fingerprint(ctx.as_ref()).is_some()
    );
}

#[tokio::test]
async fn the_extension_view_names_the_system_owner_and_this_instance() {
    let (ctx, url) = context().await;
    let view: &dyn ExtensionContext = ctx.as_ref();

    assert_eq!(view.system_owner_id(), *ctx.system_admin().id());
    assert_eq!(view.instance_id(), ctx.config().instance_id);
    assert_eq!(ExtensionContext::config(view).database_url(), url);
    assert!(view.database().is_connected());
    assert!(Arc::ptr_eq(
        &ExtensionContext::file_storage(view),
        AppContext::file_storage(ctx.as_ref())
    ));
}

#[tokio::test]
async fn the_extension_view_resolves_loaded_secrets_and_refuses_unknown_ones() {
    let (ctx, _url) = context().await;
    let view: &dyn ExtensionContext = ctx.as_ref();

    let key = view
        .secret("encryption_master_key")
        .expect("the fixture secrets carry an encryption master key");
    assert!(!key.expose().is_empty());
    assert!(view.secret("sp_no_such_secret_ctx_traits").is_none());
}

#[tokio::test]
async fn the_extension_view_finds_no_extension_that_was_never_registered() {
    let (ctx, _url) = context().await;
    let view: &dyn ExtensionContext = ctx.as_ref();

    assert!(view.get_extension("sp-no-such-extension").is_none());
    assert!(!view.has_extension("sp-no-such-extension"));
    assert_eq!(view.config_planes().is_ok(), ctx.config_planes().is_ok());
}

#[tokio::test]
async fn the_capability_traits_expose_the_shared_services() {
    let (ctx, _url) = context().await;

    let analytics = HasAnalytics::analytics(ctx.as_ref());
    let provider = AppContextTrait::analytics_provider(ctx.as_ref()).expect("analytics");
    assert!(std::ptr::addr_eq(
        Arc::as_ptr(analytics),
        Arc::as_ptr(&provider)
    ));
    let classifier = HasRouteClassifier::route_classifier(ctx.as_ref());
    assert!(matches!(
        classifier.classify("/assets/app.js", "GET"),
        RouteType::StaticAsset { .. }
    ));
}

#[tokio::test]
async fn the_extension_view_coordinates_through_the_shared_repositories() {
    let (ctx, _url) = context().await;
    let view: &dyn ExtensionContext = ctx.as_ref();
    let stranger = InstanceId::new(format!("ctx-traits-{}", uuid::Uuid::new_v4().simple()));
    let key = LeaseKey::new(format!("ctx-traits-{}", uuid::Uuid::new_v4().simple()));

    assert!(
        !view
            .instances()
            .is_alive(&stranger)
            .await
            .expect("is_alive")
    );
    assert!(view.leases().holder(&key).await.expect("holder").is_none());
    let _locks = view.advisory_locks();
}
