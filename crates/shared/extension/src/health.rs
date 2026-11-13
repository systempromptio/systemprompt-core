//! Operational hooks an extension contributes to the host: named readiness
//! checks merged into `/readyz`, diagnostic probes reported by
//! `/health/detail`, and metric descriptors registered with the Prometheus
//! recorder.
//!
//! A [`DiagnosticProbe`] never gates traffic. Its result is reported under
//! `checks.<name>` of `/health/detail`; a probe that fails or overruns the
//! probe timeout is reported there as a degraded entry carrying the error and
//! marks the detail degraded, but `/readyz` never runs it.
//!
//! A readiness check that fails, errors or overruns the probe timeout makes
//! the replica unready — the probe fails closed, because an extension that
//! cannot tell whether its dependency is up must not keep taking traffic.
//! Readiness checks are built once from the extension context when the probe
//! routes mount. `#[async_trait]` keeps [`ReadinessCheck`] `dyn`-compatible.
//!
//! A [`MetricDescriptor`] describes a metric the extension emits through the
//! `metrics` facade; a histogram's buckets must be registered before the
//! recorder installs, which only a descriptor can do.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

use async_trait::async_trait;

pub type BoxedReadinessSource = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum ReadinessFailure {
    #[error("{reason}")]
    Unready { reason: Cow<'static, str> },

    #[error("readiness check failed")]
    Failed(#[source] BoxedReadinessSource),
}

/// One named readiness signal of an extension, run on every `/readyz`.
///
/// Held as `Arc<dyn ReadinessCheck>`, hence `#[async_trait]`.
#[async_trait]
pub trait ReadinessCheck: Send + Sync {
    fn name(&self) -> &'static str;

    async fn check(&self) -> Result<(), ReadinessFailure>;
}

// JSON: a /health/detail entry is a free-form diagnostic object.
pub type DiagnosticDetail = serde_json::Map<String, serde_json::Value>;

#[derive(Debug, thiserror::Error)]
#[error("{reason}")]
pub struct DiagnosticFailure {
    pub reason: Cow<'static, str>,
    #[source]
    pub source: Option<BoxedReadinessSource>,
}

/// One named, non-gating diagnostic of an extension, run on every
/// `/health/detail`.
///
/// `probe` returns the entry's detail fields, or `None` when there is nothing
/// to report yet (the entry is omitted). `subject` names what is probed in the
/// timeout message. Held as `Arc<dyn DiagnosticProbe>`, hence `#[async_trait]`.
///
/// `/health/detail` returns a failure's `reason` and the detail fields to any
/// signed-in caller: neither may carry a secret, a credential, a connection
/// string or an underlying error's text. Put the cause in
/// `DiagnosticFailure::source`, which is only logged.
#[async_trait]
pub trait DiagnosticProbe: Send + Sync {
    fn name(&self) -> &'static str;

    fn subject(&self) -> &'static str;

    async fn probe(&self) -> Result<Option<DiagnosticDetail>, DiagnosticFailure>;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MetricKind {
    Counter,
    Gauge,
    Histogram { buckets: &'static [f64] },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricDescriptor {
    pub name: &'static str,
    pub help: &'static str,
    pub kind: MetricKind,
}
