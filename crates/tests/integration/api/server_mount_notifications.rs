//! The notification route groups on the full `setup_api_server` router: each
//! refuses a caller without credentials, and none is mounted when the
//! `notifications` extension is disabled in the profile (with the brain,
//! which requires its producer sink).

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use http::Method;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_identifiers::ExtensionId;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config, test_db_pool,
};
use tower::ServiceExt;

const GROUPS: [(Method, &str); 3] = [
    (Method::GET, "/api/v1/notifications"),
    (Method::POST, "/api/v1/admin/notifications/simulate"),
    (Method::GET, "/v1/bridge/notifications"),
];

async fn full_router(disabled: &[ExtensionId]) -> anyhow::Result<Router> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.trusted_proxies = Vec::new();
    let ctx = fixture_app_context_with_extensions(
        &pool,
        config,
        ExtensionRegistry::discover()?,
        disabled,
    )?;
    setup_api_server(&ctx)
}

fn setup_api_server(ctx: &systemprompt_runtime::AppContext) -> anyhow::Result<Router> {
    systemprompt_api::services::server::setup_api_server(
        ctx,
        None,
        &std::sync::Arc::new(systemprompt_api::services::server::ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))
}

fn request(method: Method, uri: &str) -> Request<Body> {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .expect("request build");
    req.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:40000"
            .parse::<SocketAddr>()
            .expect("peer address must parse"),
    ));
    req
}

#[tokio::test]
async fn every_notification_group_refuses_an_anonymous_caller() -> anyhow::Result<()> {
    let app = full_router(&[]).await?;
    for (method, uri) in GROUPS {
        let status = app
            .clone()
            .oneshot(request(method.clone(), uri))
            .await?
            .status();
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
    Ok(())
}

#[tokio::test]
async fn no_notification_group_is_mounted_when_the_extension_is_disabled() -> anyhow::Result<()> {
    let app = full_router(&[ExtensionId::new("notifications"), ExtensionId::new("brain")]).await?;
    for (method, uri) in GROUPS {
        let status = app
            .clone()
            .oneshot(request(method.clone(), uri))
            .await?
            .status();
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {uri}");
    }
    Ok(())
}
