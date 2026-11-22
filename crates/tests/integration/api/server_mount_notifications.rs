//! The notification route groups on the full `setup_api_server` router: each
//! refuses a caller without credentials, the admin group refuses a user
//! token, and the profile's disabled list decides mounting:
//!
//! | disables | user | admin | bridge inbox |
//! |---|---|---|---|
//! | `notifications-http` | 404 | 404 | 404 |
//! | `notifications` (with `brain`) | boot refused | | |
//! | all three | 404 | 404 | 404 |

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use http::Method;
use systemprompt_identifiers::{ExtensionId, JwtToken};
use systemprompt_models::modules::ApiPaths;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config,
    seed_bridge_credential, test_db_pool,
};
use tower::ServiceExt;

const ADMIN_SIMULATE: &str = "/api/v1/admin/notifications/simulate";

const GROUPS: [(Method, &str); 3] = [
    (Method::GET, ApiPaths::NOTIFICATIONS),
    (Method::POST, ADMIN_SIMULATE),
    (Method::GET, ApiPaths::BRIDGE_NOTIFICATIONS),
];

async fn full_router(disabled: &[ExtensionId]) -> anyhow::Result<Router> {
    let bootstrap = ensure_test_bootstrap();
    let mut config = fixture_config(&bootstrap.database_url);
    config.trusted_proxies = Vec::new();
    let ctx =
        fixture_app_context_with_extensions(config, super::common::reference_admitted(disabled)?)
            .await?;
    setup_api_server(&ctx)
}

fn setup_api_server(ctx: &systemprompt_runtime::AppContext) -> anyhow::Result<Router> {
    systemprompt_api::services::server::setup_api_server(
        ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &std::sync::Arc::new(systemprompt_api::services::server::ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))
}

fn request(method: Method, uri: &str) -> Request<Body> {
    authed(method, uri, None)
}

fn authed(method: Method, uri: &str, bearer: Option<&JwtToken>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", token.as_str()));
    }
    let mut req = builder.body(Body::from("{}")).expect("request build");
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

async fn assert_unmounted(disabled: &[ExtensionId]) -> anyhow::Result<()> {
    let app = full_router(disabled).await?;
    for (method, uri) in GROUPS {
        let status = app
            .clone()
            .oneshot(request(method.clone(), uri))
            .await?
            .status();
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{method} {uri} with {disabled:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn the_admin_group_refuses_a_user_token_at_the_admin_gate() -> anyhow::Result<()> {
    let app = full_router(&[]).await?;
    let user = seed_bridge_credential(&test_db_pool().await, "notifications-admin").await?;
    let status = app
        .oneshot(authed(Method::POST, ADMIN_SIMULATE, Some(&user.jwt)))
        .await?
        .status();
    assert_eq!(status, StatusCode::FORBIDDEN);
    Ok(())
}

#[tokio::test]
async fn disabling_the_routes_extension_unmounts_every_group_including_the_bridge_inbox()
-> anyhow::Result<()> {
    assert_unmounted(&[ExtensionId::new("notifications-http")]).await
}

#[tokio::test]
async fn disabling_notifications_and_its_routes_unmounts_every_group() -> anyhow::Result<()> {
    assert_unmounted(&[
        ExtensionId::new("notifications"),
        ExtensionId::new("notifications-http"),
        ExtensionId::new("brain"),
        ExtensionId::new("brain-http"),
    ])
    .await
}

#[tokio::test]
async fn disabling_notifications_while_its_routes_stay_enabled_refuses_boot() -> anyhow::Result<()>
{
    let refused =
        full_router(&[
            ExtensionId::new("notifications"),
            ExtensionId::new("brain"),
            ExtensionId::new("brain-http"),
        ])
        .await;
    let error = refused.expect_err("routes requiring the notification service refuse boot");
    assert!(
        format!("{error:#}").contains("NotificationsService"),
        "{error:#}"
    );
    Ok(())
}
