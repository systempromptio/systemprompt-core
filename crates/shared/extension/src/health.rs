//! Operational hooks an extension contributes to the host: named readiness
//! checks merged into `/readyz`, and metric descriptors registered with the
//! Prometheus recorder.
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
