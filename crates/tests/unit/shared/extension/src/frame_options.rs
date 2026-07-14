use axum::Router;
use axum::body::Body;
use axum::http::Request;
use axum::middleware::from_fn;
use axum::routing::get;
use systemprompt_extension::{FrameOptions, FrameOptionsOverride, stamp_frame_options};
use tower::ServiceExt;

#[test]
fn header_value_maps_policies() {
    assert_eq!(FrameOptions::Deny.header_value(), Some("DENY"));
    assert_eq!(FrameOptions::SameOrigin.header_value(), Some("SAMEORIGIN"));
    assert_eq!(FrameOptions::AllowAll.header_value(), None);
}

#[test]
fn frame_ancestors_maps_policies() {
    assert_eq!(FrameOptions::Deny.frame_ancestors(), "'none'");
    assert_eq!(FrameOptions::SameOrigin.frame_ancestors(), "'self'");
    assert_eq!(FrameOptions::AllowAll.frame_ancestors(), "*");
}

#[tokio::test]
async fn stamp_frame_options_inserts_response_extension_marker() {
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(from_fn(|req, next| {
            stamp_frame_options(FrameOptions::AllowAll, req, next)
        }));
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let marker = resp.extensions().get::<FrameOptionsOverride>();
    assert!(matches!(
        marker,
        Some(FrameOptionsOverride(FrameOptions::AllowAll))
    ));
}
