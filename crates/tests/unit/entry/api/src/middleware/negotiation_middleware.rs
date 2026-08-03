//! `content_negotiation_middleware` — the layer that turns an `Accept` header
//! into the `AcceptedFormat` extension every content handler reads.
//!
//! `parse_accept_header` is tested directly elsewhere, but the middleware that
//! installs its result is never mounted, so nothing checks that the extension
//! actually reaches the handler — which is the only thing the layer exists to
//! do.

use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::routing::get;
use axum::{Extension, Router};
use systemprompt_api::services::middleware::{
    AcceptedFormat, AcceptedMediaType, content_negotiation_middleware,
};
use tower::ServiceExt;

async fn echo_format(format: Option<Extension<AcceptedFormat>>) -> String {
    match format {
        Some(Extension(f)) => format!("{:?}", f.media_type()),
        None => "absent".to_owned(),
    }
}

async fn negotiated(accept: Option<&str>) -> String {
    let app = Router::new()
        .route("/", get(echo_format))
        .layer(axum::middleware::from_fn(content_negotiation_middleware));

    let mut builder = Request::builder().uri("/");
    if let Some(accept) = accept {
        builder = builder.header(header::ACCEPT, accept);
    }
    let resp = app
        .oneshot(builder.body(Body::empty()).expect("request must build"))
        .await
        .expect("request must complete");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 64 * 1024)
        .await
        .expect("body is small");
    String::from_utf8_lossy(&bytes).into_owned()
}

#[tokio::test]
async fn the_layer_installs_the_negotiated_format_for_the_handler() {
    assert_eq!(
        negotiated(Some("text/markdown")).await,
        format!("{:?}", AcceptedMediaType::Markdown),
        "a handler downstream of the layer must see the negotiated format"
    );
}

#[tokio::test]
async fn a_request_with_no_accept_header_still_gets_a_format() {
    assert_eq!(
        negotiated(None).await,
        format!("{:?}", AcceptedMediaType::Json),
        "the extension is always installed, so handlers need no fallback of their own"
    );
}

#[tokio::test]
async fn an_accept_header_naming_no_supported_type_falls_back_to_json() {
    assert_eq!(
        negotiated(Some("not-a-media-type")).await,
        format!("{:?}", AcceptedMediaType::Json)
    );
}

#[tokio::test]
async fn the_highest_quality_acceptable_type_wins() {
    assert_eq!(
        negotiated(Some("application/json;q=0.3, text/markdown;q=0.9")).await,
        format!("{:?}", AcceptedMediaType::Markdown),
        "q-values decide the format, not header order"
    );
}
