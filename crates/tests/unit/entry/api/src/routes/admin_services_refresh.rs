//! `POST /api/v1/admin/services/refresh`.
//!
//! The fixture profile declares no bundle sources, so a refresh resolves to
//! the baked tree. That is the shape every deployment without a services
//! source sees, and it is what the unchanged/no-restart arms assert.

use std::time::Duration;
use systemprompt_test_fixtures::AppContextSeeds as _;

use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use systemprompt_api::routes::admin::services::{RefreshLock, RefreshQuery, refresh};
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_loader::BootContext;
use systemprompt_manifest::services::root::{ActiveServicesRoot, ServicesProvenance};
use systemprompt_models::RequestContext;
use systemprompt_runtime::{AppContext, AppContextBuilder};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_admitted, fixture_config, seed_fixture_system_admin_at,
    test_app_context, test_signing_keys,
};

async fn context() -> std::sync::Arc<AppContext> {
    let boot = ensure_test_bootstrap();
    let ctx = test_app_context(&boot.database_url).await;
    ctx.db_pool().pool().close().await;
    ctx.db_pool().write_pool().close().await;
    ctx
}

async fn context_serving(root: ActiveServicesRoot) -> std::sync::Arc<AppContext> {
    let bootstrap = ensure_test_bootstrap();
    let boot = BootContext::new(
        std::sync::Arc::clone(bootstrap.boot.loaded_profile()),
        std::sync::Arc::clone(bootstrap.boot.secrets()),
    );
    boot.install_config(fixture_config(&bootstrap.database_url))
        .expect("the fixture config installs");
    boot.install_services_root(root);
    seed_fixture_system_admin_at(&bootstrap.database_url)
        .await
        .expect("the fixture admin is seeded");
    let ctx = AppContextBuilder::new(boot, fixture_admitted())
        .with_signing_keys(test_signing_keys())
        .with_services(bootstrap.services.clone())
        .build()
        .await
        .expect("a context serving the installed services root builds");
    ctx.db_pool().pool().close().await;
    ctx.db_pool().write_pool().close().await;
    std::sync::Arc::new(ctx)
}

fn req_ctx() -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("admin").expect("valid AgentName"),
        Actor::user(UserId::new("00000000-0000-4000-8000-000000000001")),
    )
}

#[tokio::test]
async fn a_profile_without_sources_refreshes_to_an_unchanged_bundled_tree() {
    let ctx = context().await;

    let body = refresh(
        State((*ctx).clone()),
        Extension(RefreshLock::default()),
        Extension(req_ctx()),
        Query(RefreshQuery { restart: false }),
    )
    .await
    .expect("a refresh with no sources succeeds")
    .0;

    assert!(
        !body.changed,
        "resolving to the baked tree is not a composition change"
    );
    assert_eq!(
        body.composed_hash, None,
        "a baked tree has no composed hash"
    );
    assert!(
        body.sources.is_empty(),
        "a profile with no sources reports no source rows"
    );
    assert!(!body.restarting);
}

#[tokio::test]
async fn restart_is_not_honoured_when_the_composition_did_not_change() {
    let ctx = context().await;

    let body = refresh(
        State((*ctx).clone()),
        Extension(RefreshLock::default()),
        Extension(req_ctx()),
        Query(RefreshQuery { restart: true }),
    )
    .await
    .expect("refresh succeeds")
    .0;

    assert!(
        !body.restarting,
        "restart=true must not bounce a process already serving the resolved tree"
    );

    let bounced = tokio::time::timeout(
        Duration::from_millis(500),
        ctx.shutdown_request().requested(),
    )
    .await;
    assert!(
        bounced.is_err(),
        "no restart was scheduled, so nothing may ask the process to shut down"
    );
}

#[tokio::test]
async fn a_refresh_already_in_flight_is_refused_rather_than_queued() {
    let ctx = context().await;
    let lock = RefreshLock::default();
    let _held = lock.try_acquire().expect("first caller takes the lock");

    let err = refresh(
        State((*ctx).clone()),
        Extension(lock),
        Extension(req_ctx()),
        Query(RefreshQuery::default()),
    )
    .await
    .expect_err("a second concurrent refresh is refused");

    assert_eq!(
        err.into_response().status(),
        StatusCode::CONFLICT,
        "the second caller must be told a refresh is running, not made to wait"
    );
}

#[tokio::test]
async fn a_composition_that_no_longer_resolves_is_a_change_and_restarts() {
    let baked = ensure_test_bootstrap().boot.require_services_root();
    let ctx = context_serving(ActiveServicesRoot {
        provenance: ServicesProvenance::Fetched {
            composed_hash: "composed-active".to_owned(),
            versions: std::collections::BTreeMap::new(),
        },
        ..baked
    })
    .await;
    assert_eq!(
        ctx.boot()
            .services_root()
            .and_then(|root| match &root.provenance {
                ServicesProvenance::Fetched { composed_hash, .. } => Some(composed_hash.as_str()),
                _ => None,
            }),
        Some("composed-active"),
        "the context serves the fetched composition the test installed"
    );

    let body = refresh(
        State((*ctx).clone()),
        Extension(RefreshLock::default()),
        Extension(req_ctx()),
        Query(RefreshQuery { restart: true }),
    )
    .await
    .expect("refresh succeeds")
    .0;

    assert!(
        body.changed,
        "a running instance whose sources no longer compose is serving a different tree"
    );
    assert!(
        body.restarting,
        "restart=true on a change schedules a bounce"
    );

    let bounced =
        tokio::time::timeout(Duration::from_secs(5), ctx.shutdown_request().requested()).await;
    assert!(
        bounced.is_ok(),
        "the scheduled restart never asked the process to shut down"
    );
}
