//! Unit tests for the MCP proxy router.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use systemprompt_mcp::services::network::proxy::create_proxy_router;
use tower::ServiceExt;

#[tokio::test]
async fn proxy_router_routes_to_unreachable_target_returns_502() {
    let router = create_proxy_router("127.0.0.1", 1);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/path?query=1")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn proxy_router_forwards_with_body() {
    let router = create_proxy_router("127.0.0.1", 2);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/x")
        .body(Body::from("payload"))
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_GATEWAY);
}
