use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use crate::config::Thresholds;

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

    pub fn check_thresholds(&self, thresholds: &Thresholds) -> bool {
        let mut passed = true;

        if self.p95().as_millis() as u64 > thresholds.p95_ms {
            eprintln!(
                "  FAIL p95 latency: {}ms > {}ms",
                self.p95().as_millis(),
                thresholds.p95_ms
            );
            passed = false;
        }

        if self.p99().as_millis() as u64 > thresholds.p99_ms {
            eprintln!(
                "  FAIL p99 latency: {}ms > {}ms",
                self.p99().as_millis(),
                thresholds.p99_ms
            );
            passed = false;
        }

        if self.error_rate() > thresholds.max_error_rate {
            eprintln!(
                "  FAIL error rate: {:.2}% > {:.2}%",
                self.error_rate() * 100.0,
                thresholds.max_error_rate * 100.0
            );
            passed = false;
        }

        passed
    }
}

pub struct Report {
    pub scenarios: BTreeMap<String, MetricsSnapshot>,
}

impl Report {
    pub fn new() -> Self {
        Self {
            scenarios: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, name: String, metrics: &Metrics) {
        self.scenarios.insert(name, metrics.snapshot());
    }

    pub fn print(&self, thresholds: &Thresholds) -> bool {
        let mut all_passed = true;

        println!("\n{:=<70}", "");
        println!("  Load Test Results");
        println!("{:=<70}\n", "");

        for (name, snapshot) in &self.scenarios {
            println!("  {name}:");
            println!("    requests:   {}", snapshot.total());
            println!("    p50:        {}ms", snapshot.p50().as_millis());
            println!("    p95:        {}ms", snapshot.p95().as_millis());
            println!("    p99:        {}ms", snapshot.p99().as_millis());
            println!("    error rate: {:.2}%", snapshot.error_rate() * 100.0);

            if !snapshot.check_thresholds(thresholds) {
                all_passed = false;
            }

            println!();
        }

        if all_passed {
            println!("  All thresholds passed.");
        } else {
            println!("  Some thresholds FAILED.");
        }

        println!("{:=<70}\n", "");
        all_passed
    }
}
