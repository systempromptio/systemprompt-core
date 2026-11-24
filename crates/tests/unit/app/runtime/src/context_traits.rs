//! `AppContext` seen through the trait objects providers hold (the
//! `systemprompt_traits::AppContext` provider accessors and the `Has*`
//! capability traits) and through the kernel seeds a member resolves by
//! type. Each must answer from the same planes as the concrete accessors.

use std::sync::Arc;

use systemprompt_extension::{HasRouteClassifier, HasUserService};
use systemprompt_identifiers::{InstanceId, LeaseKey};
use systemprompt_models::RouteType;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_database_url};
use systemprompt_traits::{
    AppContext as AppContextTrait, BackgroundTasks, DynAdvisoryLocks, DynInstanceRegistry,
    DynLeaseRepository, FileStorage, SecretStore,
};

async fn context() -> (Arc<AppContext>, String) {
    ensure_test_bootstrap();
    let url = test_database_url();
    (
        systemprompt_test_fixtures::test_app_context(&url).await,
        url,
    )
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
    assert_eq!(
        view.user_provider().is_some(),
        HasUserService::user_service(ctx.as_ref()).is_some()
    );
    assert_eq!(
        view.fingerprint_provider().is_some(),
        ctx.kernel()
            .state()
            .find::<Arc<dyn systemprompt_traits::FingerprintProvider>>()
            .is_some()
    );
}

#[tokio::test]
async fn the_kernel_seeds_name_this_instance_and_share_the_file_storage() {
    let (ctx, _url) = context().await;
    let state = ctx.kernel().state();

    assert_eq!(
        state.get::<InstanceId>().expect("instance id seed"),
        &ctx.config().instance_id
    );
    assert!(Arc::ptr_eq(
        state
            .get::<Arc<dyn FileStorage>>()
            .expect("file storage seed"),
        AppContext::file_storage(ctx.as_ref())
    ));
}

#[tokio::test]
async fn the_sealed_secret_store_resolves_loaded_secrets_and_refuses_unknown_ones() {
    let (ctx, _url) = context().await;
    let state = ctx.kernel().state();
    assert!(state.is_sealed::<Arc<dyn SecretStore>>());
    let store = state
        .get::<Arc<dyn SecretStore>>()
        .expect("the secret store is a sealed kernel seed");

    let key = store
        .secret("encryption_master_key")
        .expect("the fixture secrets carry an encryption master key");
    assert!(!key.expose().is_empty());
    assert!(store.secret("sp_no_such_secret_ctx_traits").is_none());
}

#[tokio::test]
async fn the_capability_traits_expose_the_shared_services() {
    let (ctx, _url) = context().await;

    let analytics = ctx
        .kernel()
        .state()
        .get::<Arc<dyn systemprompt_traits::AnalyticsProvider>>()
        .expect("the analytics member provides its open provider view");
    let provider = AppContextTrait::analytics_provider(ctx.as_ref());
    assert!(Arc::ptr_eq(analytics, &provider));
    let classifier = HasRouteClassifier::route_classifier(ctx.as_ref());
    assert!(matches!(
        classifier.classify("/assets/app.js", "GET"),
        RouteType::StaticAsset { .. }
    ));
}

#[tokio::test]
async fn the_coordination_seeds_answer_from_the_shared_repositories() {
    let (ctx, _url) = context().await;
    let state = ctx.kernel().state();
    let stranger = InstanceId::new(format!("ctx-traits-{}", uuid::Uuid::new_v4().simple()));
    let key = LeaseKey::new(format!("ctx-traits-{}", uuid::Uuid::new_v4().simple()));

    let instances = state.get::<DynInstanceRegistry>().expect("instances seed");
    assert!(!instances.is_alive(&stranger).await.expect("is_alive"));
    let leases = state.get::<DynLeaseRepository>().expect("leases seed");
    assert!(leases.holder(&key).await.expect("holder").is_none());
    state
        .get::<DynAdvisoryLocks>()
        .expect("advisory locks seed");
}

#[tokio::test]
async fn the_background_task_seed_shares_the_context_owner() {
    let (ctx, _url) = context().await;
    let tasks = ctx
        .kernel()
        .state()
        .get::<BackgroundTasks>()
        .expect("background tasks seed")
        .clone();
    let (release, released) = tokio::sync::oneshot::channel::<()>();
    let before = ctx.background_tasks().in_flight();

    tasks.spawn("context-traits-test", async move {
        released.await.ok();
    });

    assert_eq!(
        ctx.background_tasks().in_flight(),
        before + 1,
        "a task spawned through the seed is tracked by the context's owner"
    );
    release.send(()).expect("task still waiting");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while ctx.background_tasks().in_flight() > before && std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(
        ctx.background_tasks().in_flight(),
        before,
        "the released task finishes and leaves the owner's set"
    );
}

#[tokio::test]
async fn the_kernel_state_carries_the_oauth_repositories() {
    let (ctx, _url) = context().await;
    let state = ctx.kernel().state();
    let repos = state
        .get::<std::sync::Arc<systemprompt_oauth::repository::OAuthRepositories>>()
        .expect("the oauth repositories are a kernel state entry");
    assert!(std::sync::Arc::ptr_eq(repos, ctx.oauth_repositories()));
}

#[tokio::test]
async fn the_kernel_state_never_carries_the_analytics_broadcaster() {
    let (ctx, _url) = context().await;
    let state = ctx.kernel().state();
    assert!(
        state
            .find::<Arc<systemprompt_events::AnalyticsBroadcaster>>()
            .is_none(),
        "the analytics broadcaster is handed to its core route row, never seeded"
    );
}

#[tokio::test]
async fn the_api_build_leaves_the_documents_handles_bound() {
    let (ctx, _url) = context().await;
    let slot = ctx
        .kernel()
        .state()
        .get::<systemprompt_files::documents::DocumentsHandlesSlot>()
        .expect("the API seeds the documents handles slot");
    assert!(
        slot.get().is_some(),
        "the API binds the documents handles once its store is built"
    );
}
