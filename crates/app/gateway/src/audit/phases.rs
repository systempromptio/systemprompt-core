//! Per-phase gateway latency and the in-flight stream gauge.
//!
//! `gateway_phase_seconds{phase}` splits one inference request into
//! `admission` (quota, guards, governance, scanning and the admission commit),
//! `upstream_first_byte` (provider send until headers or a buffered body),
//! `stream` (the streamed body, until the body is dropped) and `finalize`
//! (audit completion, quota settlement and the response scan). The phase set
//! is closed, so the label cannot grow cardinality.
//!
//! `gateway_streams_in_flight` counts streamed response bodies this process is
//! still writing. A [`StreamInFlight`] is held by the body itself, so the gauge
//! covers the whole stream — including a client that disconnects mid-body —
//! where `http_requests_in_flight` stops at the response headers. It carries no
//! labels; it is the per-pod load signal a horizontal autoscaler reads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

pub const GATEWAY_PHASE_SECONDS: &str = "gateway_phase_seconds";
pub const GATEWAY_STREAMS_IN_FLIGHT: &str = "gateway_streams_in_flight";

pub const PHASE_BUCKETS: [f64; 16] = [
    0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0,
];

/// A closed set of request phases timed by `gateway_phase_seconds`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayPhase {
    Admission,
    UpstreamFirstByte,
    Stream,
    Finalize,
}

impl GatewayPhase {
    pub const ALL: [Self; 4] = [
        Self::Admission,
        Self::UpstreamFirstByte,
        Self::Stream,
        Self::Finalize,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Admission => "admission",
            Self::UpstreamFirstByte => "upstream_first_byte",
            Self::Stream => "stream",
            Self::Finalize => "finalize",
        }
    }
}

pub fn record(phase: GatewayPhase, elapsed: Duration) {
    metrics::histogram!(GATEWAY_PHASE_SECONDS, "phase" => phase.label())
        .record(elapsed.as_secs_f64());
}

/// Holds one slot of `gateway_streams_in_flight` until dropped, then records
/// the `stream` phase.
#[derive(Debug)]
pub struct StreamInFlight {
    started: Instant,
}

impl StreamInFlight {
    #[must_use]
    pub fn enter() -> Self {
        metrics::gauge!(GATEWAY_STREAMS_IN_FLIGHT).increment(1.0);
        Self {
            started: Instant::now(),
        }
    }
}

impl Drop for StreamInFlight {
    fn drop(&mut self) {
        metrics::gauge!(GATEWAY_STREAMS_IN_FLIGHT).decrement(1.0);
        record(GatewayPhase::Stream, self.started.elapsed());
    }
}
