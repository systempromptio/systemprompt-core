use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::http::header::CONTENT_TYPE;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use systemprompt_events::{
    A2A_BROADCASTER, AGUI_BROADCASTER, ANALYTICS_BROADCASTER, Broadcaster, CONTEXT_BROADCASTER,
};

const METRICS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

const HTTP_REQUESTS_TOTAL: &str = "http_requests_total";
const HTTP_REQUEST_DURATION_SECONDS: &str = "http_request_duration_seconds";
const HTTP_REQUESTS_IN_FLIGHT: &str = "http_requests_in_flight";
const SSE_CONNECTIONS: &str = "sse_active_connections";

pub fn install_recorder() -> anyhow::Result<PrometheusHandle> {
    PrometheusBuilder::new()
        .install_recorder()
        .map_err(|e| anyhow::anyhow!("failed to install Prometheus recorder: {e}"))
}

pub async fn handle_metrics(
    axum::extract::State(handle): axum::extract::State<PrometheusHandle>,
) -> Response {
    refresh_connection_gauges().await;
    let body = handle.render();
    ([(CONTENT_TYPE, METRICS_CONTENT_TYPE)], body).into_response()
}

async fn refresh_connection_gauges() {
    let context = CONTEXT_BROADCASTER.total_connections().await;
    let agui = AGUI_BROADCASTER.total_connections().await;
    let a2a = A2A_BROADCASTER.total_connections().await;
    let analytics = ANALYTICS_BROADCASTER.total_connections().await;

    metrics::gauge!(SSE_CONNECTIONS, "channel" => "context").set(context as f64);
    metrics::gauge!(SSE_CONNECTIONS, "channel" => "agui").set(agui as f64);
    metrics::gauge!(SSE_CONNECTIONS, "channel" => "a2a").set(a2a as f64);
    metrics::gauge!(SSE_CONNECTIONS, "channel" => "analytics").set(analytics as f64);
}

pub async fn track_metrics(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| req.uri().path().to_owned(), |m| m.as_str().to_owned());

    let in_flight = metrics::gauge!(HTTP_REQUESTS_IN_FLIGHT);
    in_flight.increment(1.0);

    let start = Instant::now();
    let response = next.run(req).await;
    let elapsed = start.elapsed().as_secs_f64();

    in_flight.decrement(1.0);

    let status = response.status().as_u16().to_string();
    let method = method.to_string();

    metrics::counter!(
        HTTP_REQUESTS_TOTAL,
        "method" => method.clone(),
        "path" => path.clone(),
        "status" => status.clone(),
    )
    .increment(1);
    metrics::histogram!(
        HTTP_REQUEST_DURATION_SECONDS,
        "method" => method,
        "path" => path,
        "status" => status,
    )
    .record(elapsed);

    response
}
