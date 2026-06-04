use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config::{NodeId, ScenarioId, Thresholds};

pub struct Metrics {
    inner: Mutex<MetricsInner>,
    start: Instant,
    interval_secs: u64,
}

struct IntervalBucket {
    latencies: Vec<Duration>,
    errors: u64,
    total: u64,
}

impl IntervalBucket {
    fn new() -> Self {
        Self {
            latencies: Vec::new(),
            errors: 0,
            total: 0,
        }
    }
}

struct MetricsInner {
    latencies: Vec<Duration>,
    errors: u64,
    total: u64,
    served_by: BTreeMap<String, u64>,
    buckets: Vec<IntervalBucket>,
}

impl Metrics {
    pub fn new(interval_secs: u64) -> Self {
        Self {
            inner: Mutex::new(MetricsInner {
                latencies: Vec::new(),
                errors: 0,
                total: 0,
                served_by: BTreeMap::new(),
                buckets: Vec::new(),
            }),
            start: Instant::now(),
            interval_secs,
        }
    }

    pub fn record(&self, latency: Duration, success: bool) {
        let elapsed = self.start.elapsed();
        let mut inner = self.inner.lock().expect("metrics lock poisoned");
        inner.latencies.push(latency);
        inner.total += 1;
        if !success {
            inner.errors += 1;
        }

        if let Some(bucket_idx) = elapsed.as_secs().checked_div(self.interval_secs) {
            let bucket_idx = bucket_idx as usize;
            if inner.buckets.len() <= bucket_idx {
                inner
                    .buckets
                    .resize_with(bucket_idx + 1, IntervalBucket::new);
            }
            let bucket = &mut inner.buckets[bucket_idx];
            bucket.latencies.push(latency);
            bucket.total += 1;
            if !success {
                bucket.errors += 1;
            }
        }
    }

    pub fn record_served_by(&self, instance: &str) {
        let mut inner = self.inner.lock().expect("metrics lock poisoned");
        *inner.served_by.entry(instance.to_string()).or_insert(0) += 1;
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let inner = self.inner.lock().expect("metrics lock poisoned");
        let mut latencies = inner.latencies.clone();
        latencies.sort();

        let time_series = if self.interval_secs > 0 {
            inner
                .buckets
                .iter()
                .enumerate()
                .map(|(idx, bucket)| TimeSeriesPoint::from_bucket(idx, self.interval_secs, bucket))
                .collect()
        } else {
            Vec::new()
        };

        MetricsSnapshot {
            latencies,
            errors: inner.errors,
            total: inner.total,
            served_by: inner.served_by.clone(),
            time_series,
        }
    }
}

pub struct MetricsSnapshot {
    latencies: Vec<Duration>,
    errors: u64,
    total: u64,
    served_by: BTreeMap<String, u64>,
    time_series: Vec<TimeSeriesPoint>,
}

fn percentile_of(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let idx = ((sorted.len() as f64 * p).ceil() as usize).saturating_sub(1);
    let idx = idx.min(sorted.len() - 1);
    sorted[idx]
}

impl MetricsSnapshot {
    fn percentile(&self, p: f64) -> Duration {
        percentile_of(&self.latencies, p)
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

    pub fn served_by(&self) -> &BTreeMap<String, u64> {
        &self.served_by
    }

    pub fn time_series(&self) -> &[TimeSeriesPoint] {
        &self.time_series
    }

    pub fn to_json(&self, thresholds: &Thresholds) -> ScenarioJson {
        ScenarioJson {
            requests: self.total(),
            p50_ms: self.p50().as_millis(),
            p95_ms: self.p95().as_millis(),
            p99_ms: self.p99().as_millis(),
            error_rate: self.error_rate(),
            passed: self.check_thresholds(thresholds),
            thresholds: thresholds.clone(),
            nodes: BTreeMap::new(),
            served_by: self.served_by.clone(),
            time_series: self.time_series.clone(),
        }
    }

    pub fn check_thresholds(&self, thresholds: &Thresholds) -> bool {
        (self.p95().as_millis() as u64) <= thresholds.p95_ms
            && (self.p99().as_millis() as u64) <= thresholds.p99_ms
            && self.error_rate() <= thresholds.max_error_rate
    }
}

#[derive(Clone, serde::Serialize)]
pub struct TimeSeriesPoint {
    pub window_start_secs: u64,
    pub requests: u64,
    pub error_rate: f64,
    pub p50_ms: u128,
    pub p95_ms: u128,
    pub p99_ms: u128,
}

impl TimeSeriesPoint {
    fn from_bucket(idx: usize, interval_secs: u64, bucket: &IntervalBucket) -> Self {
        let mut sorted = bucket.latencies.clone();
        sorted.sort();
        let error_rate = if bucket.total == 0 {
            0.0
        } else {
            bucket.errors as f64 / bucket.total as f64
        };
        Self {
            window_start_secs: idx as u64 * interval_secs,
            requests: bucket.total,
            error_rate,
            p50_ms: percentile_of(&sorted, 0.50).as_millis(),
            p95_ms: percentile_of(&sorted, 0.95).as_millis(),
            p99_ms: percentile_of(&sorted, 0.99).as_millis(),
        }
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
    pub thresholds: Thresholds,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub nodes: BTreeMap<String, NodeJson>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub served_by: BTreeMap<String, u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub time_series: Vec<TimeSeriesPoint>,
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
    pub thresholds: Thresholds,
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

    pub fn add(&mut self, name: ScenarioId, metrics: &Metrics, thresholds: Thresholds) {
        self.scenarios.insert(
            name,
            ScenarioReport {
                aggregate: metrics.snapshot(),
                per_node: BTreeMap::new(),
                thresholds,
            },
        );
    }

    pub fn add_distributed(
        &mut self,
        name: ScenarioId,
        per_node: &[(NodeId, MetricsSnapshot)],
        thresholds: Thresholds,
    ) {
        let mut latencies = Vec::new();
        let mut errors = 0u64;
        let mut total = 0u64;
        let mut served_by: BTreeMap<String, u64> = BTreeMap::new();
        let mut nodes = BTreeMap::new();

        for (node, snapshot) in per_node {
            latencies.extend(snapshot.latencies.iter().copied());
            errors += snapshot.errors;
            total += snapshot.total;
            for (instance, count) in &snapshot.served_by {
                *served_by.entry(instance.clone()).or_insert(0) += count;
            }
            nodes.insert(
                *node,
                MetricsSnapshot {
                    latencies: snapshot.latencies.clone(),
                    errors: snapshot.errors,
                    total: snapshot.total,
                    served_by: snapshot.served_by.clone(),
                    time_series: Vec::new(),
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
                    served_by,
                    time_series: Vec::new(),
                },
                per_node: nodes,
                thresholds,
            },
        );
    }
}
