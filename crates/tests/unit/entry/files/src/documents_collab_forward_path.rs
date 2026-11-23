//! The path a collab update is forwarded on keeps the documents prefix that
//! the nested router strips from `Request::uri`.

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::routing::post;
use systemprompt_api::routes::documents::collab_updates::forward_path_and_query;
use tower::ServiceExt;

async fn echo(request: Request) -> String {
    forward_path_and_query(&request)
}

async fn forwarded_path(uri: &str) -> String {
    let router = Router::new().nest(
        "/api/v1/documents",
        Router::new().route("/{id}/collab/updates", post(echo)),
    );
    let response = router
        .oneshot(Request::post(uri).body(Body::empty()).expect("request"))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .expect("body");
    String::from_utf8(body.to_vec()).expect("utf-8")
}

#[tokio::test]
async fn a_nested_route_forwards_the_full_path() {
    assert_eq!(
        forwarded_path("/api/v1/documents/doc-1/collab/updates").await,
        "/api/v1/documents/doc-1/collab/updates"
    );
}

#[tokio::test]
async fn the_query_string_travels_with_the_path() {
    assert_eq!(
        forwarded_path("/api/v1/documents/doc-1/collab/updates?session_id=s&vv=AA").await,
        "/api/v1/documents/doc-1/collab/updates?session_id=s&vv=AA"
    );
}
