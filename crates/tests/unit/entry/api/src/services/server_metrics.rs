//! The Prometheus recorder, the `/metrics` router, and the standalone metrics
//! listener.
//!
//! The recorder is a process global — `metrics::set_global_recorder` is a
//! one-shot — so the interesting property is that a second caller is handed the
//! first handle instead of an error. The listener is the other half: it is
//! bound outside the public router, on an address nothing else asserts is
//! served.

use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use systemprompt_api::services::server::metrics::{
    install_recorder, metrics_router, serve_metrics_listener,
};
use systemprompt_test_fixtures::free_port_in_range;
use tower::ServiceExt;

#[tokio::test]
async fn installing_the_recorder_twice_hands_back_the_first_handle() {
    install_recorder("fixture-instance").expect("first install succeeds");
    let second = install_recorder("a-different-instance")
        .expect("a second install must not be a hard error — the recorder is a process global");

    metrics::counter!("second_install_probe_total").increment(1);
    let rendered = second.render();

    assert!(
        rendered.contains("second_install_probe_total"),
        "the handle handed to the second caller must render the live recorder, not an empty one"
    );
    assert!(
        !rendered.contains("a-different-instance"),
        "the second call's instance label must be ignored — the first recorder is the one \
         installed globally, and a second set_global_recorder would be a hard error"
    );
}

#[tokio::test]
async fn the_metrics_route_renders_the_prometheus_exposition_format() {
    let handle = install_recorder("fixture-instance").expect("recorder installs");
    metrics::counter!("test_render_probe_total").increment(1);

    let response = metrics_router(handle)
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("router answers");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("text/plain; version=0.0.4; charset=utf-8"),
        "scrapers select the parser from this content type"
    );

    let body = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body reads");
    let text = String::from_utf8(body.to_vec()).expect("exposition output is UTF-8");
    assert!(
        text.contains("sse_active_connections"),
        "the handler refreshes the SSE gauges before rendering; got: {text}"
    );
}

// Why: `/metrics` is bound on its own listener rather than mounted on the
// public router, so a scraper reaches it without a bearer token. Nothing else
// asserts that this address is actually served.
#[tokio::test]
async fn the_metrics_listener_binds_and_serves_its_own_address() {
    let handle = install_recorder("fixture-instance").expect("recorder installs");
    let port = free_port_in_range(19_400..19_500).expect("a free port");
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));

    serve_metrics_listener(addr, handle)
        .await
        .expect("the listener binds");

    let connected = tokio::net::TcpStream::connect(addr).await;
    assert!(
        connected.is_ok(),
        "the metrics listener must accept connections on its own address: {connected:?}"
    );
}

#[tokio::test]
async fn a_port_already_held_is_reported_rather_than_silently_unserved() {
    let handle = install_recorder("fixture-instance").expect("recorder installs");
    let held = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind a port to hold");
    let addr = held.local_addr().expect("held address");

    let result = serve_metrics_listener(addr, handle).await;

    assert!(
        result.is_err(),
        "binding an occupied port must surface as an error, not a listener that never serves"
    );
}
