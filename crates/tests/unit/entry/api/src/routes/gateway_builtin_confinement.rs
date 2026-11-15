//! The built-in gateway surface answers only under the kernel's built-in
//! prefixes, matched segment-bounded: a sibling path that merely starts with
//! a prefix, and any path outside every prefix, answer 404.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use systemprompt_api::routes::gateway::builtin::confined;
use tower::ServiceExt;

async fn ok() -> &'static str {
    "ok"
}

fn surface() -> Router {
    confined(
        Router::new()
            .route("/", get(ok))
            .route("/models", get(ok))
            .route("/models/{id}", get(ok))
            .route("/modelsx", get(ok))
            .route("/unlisted", get(ok)),
    )
}

async fn status(path: &str) -> StatusCode {
    surface()
        .oneshot(Request::get(path).body(Body::empty()).expect("request"))
        .await
        .expect("the router answers")
        .status()
}

#[tokio::test]
async fn a_path_under_a_builtin_prefix_is_served() {
    assert_eq!(status("/models").await, StatusCode::OK);
    assert_eq!(status("/models/claude").await, StatusCode::OK);
    assert_eq!(status("/").await, StatusCode::OK);
}

#[tokio::test]
async fn a_sibling_of_a_prefix_is_not_under_it() {
    assert_eq!(status("/modelsx").await, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_path_outside_every_prefix_answers_404() {
    assert_eq!(status("/unlisted").await, StatusCode::NOT_FOUND);
}
