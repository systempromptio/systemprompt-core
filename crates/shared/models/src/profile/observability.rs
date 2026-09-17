//! Where this instance ships its own telemetry.
//!
//! The gateway already *ingests* OTLP; this block is the other direction: the
//! `otlp_export` job tails the audit tables and posts every completed AI
//! request as a span (tool calls and governance decisions as its children) and
//! every `logs` row as a log record to the configured collector. Metrics are
//! not exported here — they stay on the Prometheus `/metrics` listener.
//!
//! `endpoint` is the collector base URL (OTLP/HTTP appends `/v1/traces` and
//! `/v1/logs`); `headers` go verbatim on every export request; `signals`
//! selects what is shipped; `batch_seconds` is the minimum spacing between
//! two exports of one signal — the job's cron tick is the upper bound on how
//! often it runs, this the lower bound on how often it ships.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObservabilityConfig {
    #[serde(default)]
    pub otlp: Option<OtlpExportConfig>,
}

impl ObservabilityConfig {
    #[must_use]
    pub const fn otlp(&self) -> Option<&OtlpExportConfig> {
        self.otlp.as_ref()
    }
}

/// An OTLP collector to export traces and logs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OtlpExportConfig {
    pub endpoint: String,

    #[serde(default)]
    pub protocol: OtlpProtocol,

    #[serde(default)]
    pub headers: BTreeMap<String, String>,

    #[serde(default = "default_signals")]
    pub signals: Vec<OtlpSignal>,

    #[serde(default = "default_batch_seconds")]
    pub batch_seconds: u64,
}

impl OtlpExportConfig {
    pub const DEFAULT_BATCH_SECONDS: u64 = 15;
    pub const MIN_BATCH_SECONDS: u64 = 1;
    pub const MAX_BATCH_SECONDS: u64 = 3600;

    #[must_use]
    pub fn exports(&self, signal: OtlpSignal) -> bool {
        self.signals.contains(&signal)
    }

    // Why: OTLP/HTTP (opentelemetry-proto, "OTLP/HTTP request") fixes the
    // per-signal path: `/v1/traces`, `/v1/logs`. An endpoint that already
    // ends in that path is used as given.
    #[must_use]
    pub fn signal_url(&self, signal: OtlpSignal) -> String {
        let base = self.endpoint.trim_end_matches('/');
        let path = signal.http_path();
        if base.ends_with(path) {
            base.to_owned()
        } else {
            format!("{base}{path}")
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum OtlpProtocol {
    #[default]
    Http,
    Grpc,
}

impl OtlpProtocol {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Grpc => "grpc",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OtlpSignal {
    Traces,
    Logs,
}

impl OtlpSignal {
    pub const ALL: [Self; 2] = [Self::Traces, Self::Logs];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Traces => "traces",
            Self::Logs => "logs",
        }
    }

    #[must_use]
    pub const fn http_path(self) -> &'static str {
        match self {
            Self::Traces => "/v1/traces",
            Self::Logs => "/v1/logs",
        }
    }

    #[must_use]
    pub fn parse(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.label() == label)
    }
}

impl std::fmt::Display for OtlpSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

fn default_signals() -> Vec<OtlpSignal> {
    OtlpSignal::ALL.to_vec()
}

const fn default_batch_seconds() -> u64 {
    OtlpExportConfig::DEFAULT_BATCH_SECONDS
}
