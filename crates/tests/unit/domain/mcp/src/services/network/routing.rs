//! Behaviour tests for the network routing helpers: the base health route, the
//! CORS layer built from the bootstrap config, and `/mcp` nesting.

use axum::body::Body;
use http::Request;
use systemprompt_mcp::services::network::routing::{
    add_middleware, apply_cors_layer, create_base_router, create_mcp_router,
};
use systemprompt_test_fixtures::ensure_test_bootstrap;
use tower::ServiceExt;

async fn body_string(response: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .expect("body bytes");
    String::from_utf8(bytes.to_vec()).expect("utf8 body")
}

#[tokio::test]
async fn base_router_serves_health_check() {
    let response = create_base_router()
        .oneshot(
            Request::get("/health")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(body_string(response).await, "OK");
}

#[tokio::test]
async fn cors_layer_allows_configured_origin() {
    ensure_test_bootstrap();
    let router = apply_cors_layer(create_base_router()).expect("cors layer built");

    let response = router
        .oneshot(
            Request::get("/health")
                .header("origin", "http://127.0.0.1")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("http://127.0.0.1")
    );
}

#[tokio::test]
async fn mcp_router_nests_under_mcp_prefix() {
    let mcp = axum::Router::new().route("/", axum::routing::get(|| async { "nested" }));
    let composed = add_middleware(create_mcp_router(create_base_router(), mcp));

    let response = composed
        .oneshot(Request::get("/mcp").body(Body::empty()).expect("request"))
        .await
        .expect("response");
    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(body_string(response).await, "nested");
}
