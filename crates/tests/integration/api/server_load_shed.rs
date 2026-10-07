//! Overload shedding on a router carrying the real probes and `/metrics`.
//!
//! With a ceiling of one in-flight request, a second request is refused at
//! once with `503`, `Retry-After: 1` and the `overloaded` error key, while the
//! probes and the scrape endpoint keep answering and `/readyz` reports the
//! replica saturated. Releasing the held request restores admission.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use systemprompt_api::services::middleware::LoadShed;
use systemprompt_api::services::middleware::load_shed::shed;
use systemprompt_api::services::server::metrics::{install_recorder, metrics_router};
use systemprompt_api::services::server::{discovery_router, signal_ready};
use systemprompt_identifiers::InstanceId;
use systemprompt_test_fixtures::{test_app_context, test_database_url, test_db_pool};
use tokio::sync::Notify;
use tower::ServiceExt;

async fn app(limit: Arc<LoadShed>, release: Arc<Notify>, entered: Arc<Notify>) -> Router {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let ctx = (*test_app_context(&pool, &url)).clone();
    let handle = install_recorder(&InstanceId::new("load-shed-fixture")).expect("recorder");
    let slow = axum::routing::get(move || {
        let release = Arc::clone(&release);
        let entered = Arc::clone(&entered);
        async move {
            entered.notify_one();
            release.notified().await;
            "done"
        }
    });
    discovery_router(&ctx)
        .merge(metrics_router(handle))
        .route("/slow", slow)
        .layer(axum::middleware::from_fn_with_state(limit, shed))
}

async fn get(app: &Router, uri: &str) -> (StatusCode, Option<String>, serde_json::Value, String) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = resp.status();
    let retry_after = resp
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = axum::body::to_bytes(resp.into_body(), 256 * 1024)
        .await
        .expect("body");
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let json = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    (status, retry_after, json, text)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_saturated_replica_sheds_requests_but_keeps_answering_probes() {
    signal_ready();
    let limit = Arc::new(LoadShed::new(NonZeroU32::new(1).expect("non-zero")));
    let release = Arc::new(Notify::new());
    let entered = Arc::new(Notify::new());
    let app = app(
        Arc::clone(&limit),
        Arc::clone(&release),
        Arc::clone(&entered),
    )
    .await;

    assert!((limit.saturation() - 0.0).abs() < f64::EPSILON);
    let held = tokio::spawn({
        let app = app.clone();
        async move { get(&app, "/slow").await.0 }
    });
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .expect("the first request reaches the handler");
    assert!(limit.saturated());
    assert!((limit.saturation() - 1.0).abs() < f64::EPSILON);

    let (status, retry_after, body, _) = get(&app, "/slow").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(retry_after.as_deref(), Some("1"));
    assert_eq!(body["error_key"], "overloaded", "{body}");

    assert_eq!(get(&app, "/livez").await.0, StatusCode::OK);
    let (ready, _, ready_body, _) = get(&app, "/readyz").await;
    assert_eq!(ready, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(ready_body["status"], "saturated", "{ready_body}");
    assert_eq!(ready_body["in_flight"], 1);
    assert_eq!(ready_body["limit"], 1);
    let (scrape, _, _, metrics) = get(&app, "/metrics").await;
    assert_eq!(scrape, StatusCode::OK);
    assert!(metrics.contains("http_load_shed_total"), "{metrics}");
    assert!(metrics.contains("http_in_flight_saturation"), "{metrics}");

    release.notify_one();
    assert_eq!(held.await.expect("held request"), StatusCode::OK);
    assert!(!limit.saturated());
    assert!((limit.saturation() - 0.0).abs() < f64::EPSILON);

    let next = tokio::spawn({
        let app = app.clone();
        async move { get(&app, "/slow").await.0 }
    });
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .expect("admission is restored");
    release.notify_one();
    assert_eq!(next.await.expect("next request"), StatusCode::OK);
    assert_eq!(get(&app, "/readyz").await.0, StatusCode::OK);
}
