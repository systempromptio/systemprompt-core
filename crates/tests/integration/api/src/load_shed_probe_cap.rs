//! The probe ceiling: health, readiness and metrics routes bypass the main
//! in-flight ceiling but share their own. A flood of probes saturates only
//! the probe ceiling (refused with the same `503` shape) while ordinary
//! requests are still admitted, and a saturated main ceiling never refuses a
//! liveness probe.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use systemprompt_api::services::middleware::load_shed::{InFlightLimits, outermost};
use tokio::sync::Notify;
use tower::ServiceExt;

fn held(release: Arc<Notify>, entered: Arc<Notify>) -> axum::routing::MethodRouter {
    axum::routing::get(move || {
        let release = Arc::clone(&release);
        let entered = Arc::clone(&entered);
        async move {
            entered.notify_one();
            release.notified().await;
            "done"
        }
    })
}

struct Gate {
    release: Arc<Notify>,
    entered: Arc<Notify>,
}

impl Gate {
    fn new() -> Self {
        Self {
            release: Arc::new(Notify::new()),
            entered: Arc::new(Notify::new()),
        }
    }

    fn route(&self) -> axum::routing::MethodRouter {
        held(Arc::clone(&self.release), Arc::clone(&self.entered))
    }
}

fn app(probe: &Gate, work: &Gate) -> Router {
    let router = Router::new()
        .route("/metrics", probe.route())
        .route("/livez", axum::routing::get(|| async { "live" }))
        .route("/work", work.route())
        .route("/quick", axum::routing::get(|| async { "quick" }));
    outermost(
        router,
        InFlightLimits {
            requests: NonZeroU32::new(1),
            probes: NonZeroU32::new(1).expect("non-zero"),
        },
    )
}

async fn get(app: &Router, uri: &str) -> (StatusCode, Option<String>, serde_json::Value) {
    let resp = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).expect("request"))
        .await
        .expect("response");
    let status = resp.status();
    let retry_after = resp
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .expect("body");
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, retry_after, json)
}

async fn hold(app: &Router, gate: &Gate, uri: &'static str) -> tokio::task::JoinHandle<StatusCode> {
    let task = tokio::spawn({
        let app = app.clone();
        async move { get(&app, uri).await.0 }
    });
    tokio::time::timeout(Duration::from_secs(5), gate.entered.notified())
        .await
        .expect("the held request reaches its handler");
    task
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_probe_flood_saturates_only_the_probe_ceiling() {
    let (probe, work) = (Gate::new(), Gate::new());
    let app = app(&probe, &work);

    let scrape = hold(&app, &probe, "/metrics").await;
    let (status, retry_after, body) = get(&app, "/livez").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(retry_after.as_deref(), Some("1"));
    assert_eq!(body["error_key"], "overloaded", "{body}");
    assert_eq!(
        get(&app, "/quick").await.0,
        StatusCode::OK,
        "a saturated probe ceiling leaves the main ceiling free"
    );

    probe.release.notify_one();
    assert_eq!(scrape.await.expect("held scrape"), StatusCode::OK);
    assert_eq!(
        get(&app, "/livez").await.0,
        StatusCode::OK,
        "the probe permit is returned"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_saturated_main_ceiling_never_fails_liveness() {
    let (probe, work) = (Gate::new(), Gate::new());
    let app = app(&probe, &work);

    let held_work = hold(&app, &work, "/work").await;
    let (status, _, body) = get(&app, "/quick").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["error_key"], "overloaded", "{body}");
    assert_eq!(
        get(&app, "/livez").await.0,
        StatusCode::OK,
        "liveness is answered while the main ceiling is saturated"
    );

    work.release.notify_one();
    assert_eq!(held_work.await.expect("held work"), StatusCode::OK);
}
