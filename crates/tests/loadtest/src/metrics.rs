use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use crate::config::{NodeId, ScenarioId, Thresholds};

pub struct Metrics {
    inner: Mutex<MetricsInner>,
}

struct MetricsInner {
    latencies: Vec<Duration>,
    errors: u64,
    total: u64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MetricsInner {
                latencies: Vec::new(),
                errors: 0,
                total: 0,
            }),
        }
    }

    pub fn record(&self, latency: Duration, success: bool) {
        let mut inner = self.inner.lock().expect("metrics lock poisoned");
        inner.latencies.push(latency);
        inner.total += 1;
        if !success {
            inner.errors += 1;
        }
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let inner = self.inner.lock().expect("metrics lock poisoned");
        let mut latencies = inner.latencies.clone();
        latencies.sort();
        MetricsSnapshot {
            latencies,
            errors: inner.errors,
            total: inner.total,
        }
    }
}

pub struct MetricsSnapshot {
    latencies: Vec<Duration>,
    errors: u64,
    total: u64,
}

impl MetricsSnapshot {
    fn percentile(&self, p: f64) -> Duration {
        if self.latencies.is_empty() {
            return Duration::ZERO;
        }
        let idx = ((self.latencies.len() as f64 * p).ceil() as usize).saturating_sub(1);
        let idx = idx.min(self.latencies.len() - 1);
        self.latencies[idx]
    }

    pub fn p50(&self) -> Duration {
        self.percentile(0.50)
    }

    pub fn p95(&self) -> Duration {
        self.percentile(0.95)
    }

    pub fn p99(&self) -> Duration {
        self.percentile(0.99)
    }

    pub fn error_rate(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.errors as f64 / self.total as f64
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn to_json(&self, thresholds: &Thresholds) -> ScenarioJson {
        ScenarioJson {
            requests: self.total(),
            p50_ms: self.p50().as_millis(),
            p95_ms: self.p95().as_millis(),
            p99_ms: self.p99().as_millis(),
            error_rate: self.error_rate(),
            passed: self.check_thresholds(thresholds),
            nodes: BTreeMap::new(),
        }
    }

    pub fn check_thresholds(&self, thresholds: &Thresholds) -> bool {
        (self.p95().as_millis() as u64) <= thresholds.p95_ms
            && (self.p99().as_millis() as u64) <= thresholds.p99_ms
            && self.error_rate() <= thresholds.max_error_rate
    }
}

#[derive(serde::Serialize)]
pub struct ScenarioJson {
    pub requests: u64,
    pub p50_ms: u128,
    pub p95_ms: u128,
    pub p99_ms: u128,
    pub error_rate: f64,
    pub passed: bool,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub nodes: BTreeMap<String, NodeJson>,
}

#[derive(serde::Serialize)]
pub struct NodeJson {
    pub requests: u64,
    pub p50_ms: u128,
    pub p95_ms: u128,
    pub p99_ms: u128,
    pub error_rate: f64,
}

impl MetricsSnapshot {
    pub fn to_node_json(&self) -> NodeJson {
        NodeJson {
            requests: self.total(),
            p50_ms: self.p50().as_millis(),
            p95_ms: self.p95().as_millis(),
            p99_ms: self.p99().as_millis(),
            error_rate: self.error_rate(),
        }
    }
}

pub struct ScenarioReport {
    pub aggregate: MetricsSnapshot,
    pub per_node: BTreeMap<NodeId, MetricsSnapshot>,
}

pub struct Report {
    pub scenarios: BTreeMap<ScenarioId, ScenarioReport>,
}

impl Report {
    pub fn new() -> Self {
        Self {
            scenarios: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, name: ScenarioId, metrics: &Metrics) {
        self.scenarios.insert(
            name,
            ScenarioReport {
                aggregate: metrics.snapshot(),
                per_node: BTreeMap::new(),
            },
        );
    }

    pub fn add_distributed(&mut self, name: ScenarioId, per_node: &[(NodeId, MetricsSnapshot)]) {
        let mut latencies = Vec::new();
        let mut errors = 0u64;
        let mut total = 0u64;
        let mut nodes = BTreeMap::new();

        for (node, snapshot) in per_node {
            latencies.extend(snapshot.latencies.iter().copied());
            errors += snapshot.errors;
            total += snapshot.total;
            nodes.insert(
                *node,
                MetricsSnapshot {
                    latencies: snapshot.latencies.clone(),
                    errors: snapshot.errors,
                    total: snapshot.total,
                },
            );
        }
        latencies.sort();

        self.scenarios.insert(
            name,
            ScenarioReport {
                aggregate: MetricsSnapshot {
                    latencies,
                    errors,
                    total,
                },
                per_node: nodes,
            },
        );
    }
}
