//! The `marketplace-http` consumer routes as the full server mounts them:
//! with `gateway-http` admitted the device-enrolment route is served and
//! refuses a caller without a device credential; with `gateway-http` disabled
//! there is no managed consumer to build them from, so they are not mounted
//! (404) although `marketplace-http` stays admitted.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, header};
use systemprompt_api::services::server::setup_api_server;
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::modules::ApiPaths;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_admitted, fixture_admitted_without,
    fixture_app_context_with_extensions, fixture_config,
};
use tower::ServiceExt;

async fn server(admitted: Arc<systemprompt_extension::Admitted>) -> Router {
    let boot = ensure_test_bootstrap();
    let ctx = fixture_app_context_with_extensions(fixture_config(&boot.database_url), admitted)
        .await
        .expect("context builds");
    setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(systemprompt_api::services::server::ReadinessState::new()),
    )
    .expect("server builds")
}

async fn enrol(app: &Router) -> u16 {
    let mut req = Request::builder()
        .method(Method::POST)
        .uri(ApiPaths::CONSUMER_DEVICE_ENROLLMENT)
        .header("host", "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .expect("request");
    req.extensions_mut().insert(ConnectInfo(
        "203.0.113.7:40000"
            .parse::<std::net::SocketAddr>()
            .expect("addr"),
    ));
    app.clone()
        .oneshot(req)
        .await
        .expect("response")
        .status()
        .as_u16()
}

#[tokio::test]
async fn the_consumer_routes_are_served_when_gateway_http_is_admitted() {
    let app = server(fixture_admitted()).await;
    assert_ne!(enrol(&app).await, 404, "the enrolment route is mounted");
}

#[tokio::test]
async fn the_consumer_routes_are_not_mounted_without_gateway_http() {
    let app = server(fixture_admitted_without(&[ExtensionId::new("gateway-http")])).await;
    assert_eq!(enrol(&app).await, 404);
}
