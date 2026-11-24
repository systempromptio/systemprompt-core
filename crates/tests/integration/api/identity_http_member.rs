//! The `identity-http` member's surface as the full server mounts it: with
//! the member admitted its OAuth, discovery, session and API-key routes are
//! served (an anonymous caller is refused where a session is required); with
//! it disabled none of them is mounted (404).

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request};
use systemprompt_api::services::server::setup_api_server;
use systemprompt_identifiers::ExtensionId;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_admitted, fixture_admitted_without,
    fixture_app_context_with_extensions, fixture_config,
};
use tower::ServiceExt;

const PUBLIC: [(&str, &str); 3] = [
    ("GET", "/.well-known/oauth-authorization-server"),
    ("GET", "/api/v1/core/oauth/health"),
    ("GET", "/auth/link-passkey"),
];
const GATED: [(&str, &str); 4] = [
    ("GET", "/api/v1/core/oauth/userinfo"),
    ("POST", "/api/v1/core/oauth/revoke"),
    ("POST", "/api/v1/core/users/me/sessions/revoke_all"),
    ("GET", "/api/v1/admin/api-keys"),
];

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
    .layer(axum::Extension(Arc::new(
        systemprompt_api::services::server::SchedulerHealth::new(),
    )))
}

async fn status(app: &Router, method: &str, uri: &str) -> u16 {
    let mut req = Request::builder()
        .method(Method::from_bytes(method.as_bytes()).expect("method"))
        .uri(uri)
        .header("host", "127.0.0.1")
        .header("user-agent", "identity-member-test/1.0")
        .body(Body::empty())
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
async fn the_admitted_member_serves_its_routes_behind_their_gates() {
    let app = server(fixture_admitted()).await;
    for (method, uri) in PUBLIC {
        assert_eq!(status(&app, method, uri).await, 200, "{uri}");
    }
    for (method, uri) in GATED {
        assert_eq!(status(&app, method, uri).await, 401, "{uri}");
    }
}

#[tokio::test]
async fn a_disabled_identity_http_member_mounts_none_of_its_routes() {
    let app = server(fixture_admitted_without(&[ExtensionId::new(
        "identity-http",
    )]))
    .await;
    for (method, uri) in PUBLIC.into_iter().chain(GATED) {
        assert_ne!(status(&app, method, uri).await, 200, "{uri}");
        assert_ne!(
            status(&app, method, uri).await,
            401,
            "{uri} is not gated: not mounted"
        );
    }
}
