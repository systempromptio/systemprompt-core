//! Forwarding-path tests for the MCP proxy router against a live wiremock
//! target: method, path, query, headers, and body must all pass through, and
//! the upstream status and body must come back verbatim.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use systemprompt_mcp::services::network::proxy::create_proxy_router;
use tower::ServiceExt;
use wiremock::matchers::{body_string, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn proxy_forwards_post_with_query_headers_and_body() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/echo"))
        .and(query_param("q", "1"))
        .and(header("x-custom", "abc"))
        .and(body_string("payload"))
        .respond_with(ResponseTemplate::new(201).set_body_raw("created", "text/plain"))
        .mount(&mock)
        .await;

    let router = create_proxy_router("127.0.0.1", mock.address().port());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/echo?q=1")
        .header("x-custom", "abc")
        .header("host", "should-be-stripped")
        .body(Body::from("payload"))
        .unwrap();

    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"created");

    let received = mock.received_requests().await.expect("requests recorded");
    assert_eq!(received.len(), 1);
    assert_ne!(
        received[0]
            .headers
            .get("host")
            .map(|v| v.to_str().unwrap_or_default()),
        Some("should-be-stripped"),
        "the inbound host header is not forwarded"
    );
}

#[tokio::test]
async fn proxy_forwards_get_without_body_and_returns_upstream_status() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&mock)
        .await;

    let router = create_proxy_router("127.0.0.1", mock.address().port());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn proxy_passes_through_upstream_error_statuses() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_raw("nope", "text/plain"))
        .mount(&mock)
        .await;

    let router = create_proxy_router("127.0.0.1", mock.address().port());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/missing")
        .body(Body::empty())
        .unwrap();

    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"nope");
}

#[tokio::test]
async fn proxy_preserves_upstream_response_headers_and_strips_hop_by_hop() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .insert_header("mcp-session-id", "abc123")
                .set_body_raw("data: {}\n\n", "text/event-stream"),
        )
        .mount(&mock)
        .await;

    let router = create_proxy_router("127.0.0.1", mock.address().port());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/mcp")
        .body(Body::empty())
        .unwrap();

    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream"),
        "upstream response headers must survive the proxy hop"
    );
    assert_eq!(
        res.headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok()),
        Some("abc123")
    );
    assert!(
        res.headers().get("transfer-encoding").is_none(),
        "hop-by-hop headers must not be forwarded"
    );
}

#[tokio::test]
async fn proxy_forwards_sep_2243_operation_headers() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header("mcp-method", "tools/call"))
        .and(header("mcp-name", "list_users"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("{}", "application/json"))
        .mount(&mock)
        .await;

    let router = create_proxy_router("127.0.0.1", mock.address().port());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/mcp")
        .header("mcp-method", "tools/call")
        .header("mcp-name", "list_users")
        .body(Body::from("{}"))
        .unwrap();

    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
