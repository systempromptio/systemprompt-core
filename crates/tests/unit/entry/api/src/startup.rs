use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use systemprompt_api::services::server::{bind_and_serve, starting_router};
use tower::ServiceExt;

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("json body")
}

#[tokio::test]
async fn starting_router_reports_starting_on_api_health_path() {
    let response = starting_router()
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await["status"], "starting");
}

#[tokio::test]
async fn starting_router_reports_starting_on_bare_health_path() {
    let response = starting_router()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await["status"], "starting");
}

#[tokio::test]
async fn starting_router_returns_503_for_other_paths() {
    let response = starting_router()
        .oneshot(
            Request::builder()
                .uri("/api/v1/agents")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body_json(response).await["error"], "service starting");
}

#[tokio::test]
async fn early_server_swaps_full_router_on_same_listener() {
    let early = bind_and_serve("127.0.0.1:0", None).await.expect("bind");
    let addr = early.local_addr();

    let starting = http_get(addr, "/api/v1/health").await;
    assert!(starting.contains("starting"), "got: {starting}");

    let full = Router::new().route("/api/v1/health", get(|| async { "healthy" }));
    early.activate(full);

    let swapped = http_get(addr, "/api/v1/health").await;
    assert!(swapped.contains("healthy"), "got: {swapped}");
}

async fn http_get(addr: std::net::SocketAddr, path: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut stream = tokio::net::TcpStream::connect(addr).await.expect("connect");
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await
        .expect("write request");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .await
        .expect("read response");
    response
}
