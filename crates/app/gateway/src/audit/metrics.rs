//! Prometheus histograms for gateway latency.
//!
//! `gateway_upstream_duration_seconds` is the provider call alone;
//! `gateway_overhead_seconds` is the time the gateway itself added (the whole
//! request minus the upstream bracket). Both are recorded once per completed
//! request from [`super::GatewayAudit::complete`], the single sink for the
//! buffered and the streamed path, labelled by inbound wire dialect (`route`)
//! and the provider that served the response. The bucket bounds are exported
//! so the recorder installs them explicitly: the overhead histogram needs
//! millisecond resolution, the upstream one spans minutes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const GATEWAY_OVERHEAD_SECONDS: &str = "gateway_overhead_seconds";
pub const GATEWAY_UPSTREAM_DURATION_SECONDS: &str = "gateway_upstream_duration_seconds";

pub const OVERHEAD_BUCKETS: [f64; 10] =
    [0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0];
pub const UPSTREAM_BUCKETS: [f64; 11] =
    [0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 30.0, 60.0, 120.0];

pub fn record_completion(route: &str, provider: &str, latency_ms: i32, upstream_ms: Option<i32>) {
    let Some(upstream_ms) = upstream_ms else {
        return;
    };
    let overhead_ms = latency_ms.saturating_sub(upstream_ms).max(0);
    metrics::histogram!(
        GATEWAY_UPSTREAM_DURATION_SECONDS,
        "route" => route.to_owned(),
        "provider" => provider.to_owned(),
    )
    .record(f64::from(upstream_ms.max(0)) / 1000.0);
    metrics::histogram!(
        GATEWAY_OVERHEAD_SECONDS,
        "route" => route.to_owned(),
        "provider" => provider.to_owned(),
    )
    .record(f64::from(overhead_ms) / 1000.0);
}
