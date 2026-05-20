use std::collections::BTreeMap;

use serde::Serialize;

use crate::config::Thresholds;
use crate::metrics::{Report, ScenarioJson};

#[derive(Serialize)]
struct ThresholdsJson {
    p95_ms: u64,
    p99_ms: u64,
    max_error_rate: f64,
}

#[derive(Serialize)]
struct Aggregate {
    total_requests: u64,
    all_passed: bool,
}

#[derive(Serialize)]
struct JsonReport {
    thresholds: ThresholdsJson,
    scenarios: BTreeMap<String, ScenarioJson>,
    aggregate: Aggregate,
}

pub fn write(report: &Report, thresholds: &Thresholds, out_file: &str) -> Result<bool, String> {
    let mut scenarios = BTreeMap::new();
    let mut total_requests = 0u64;
    let mut all_passed = true;

    for (name, scenario) in &report.scenarios {
        let mut record = scenario.aggregate.to_json(thresholds);
        for (node, snapshot) in &scenario.per_node {
            record
                .nodes
                .insert(node.to_string(), snapshot.to_node_json());
        }
        total_requests += record.requests;
        if !record.passed {
            all_passed = false;
        }
        scenarios.insert(name.to_string(), record);
    }

    let json_report = JsonReport {
        thresholds: ThresholdsJson {
            p95_ms: thresholds.p95_ms,
            p99_ms: thresholds.p99_ms,
            max_error_rate: thresholds.max_error_rate,
        },
        scenarios,
        aggregate: Aggregate {
            total_requests,
            all_passed,
        },
    };

    let serialized = serde_json::to_string_pretty(&json_report)
        .map_err(|e| format!("failed to serialize JSON report: {e}"))?;

    std::fs::write(out_file, serialized)
        .map_err(|e| format!("failed to write JSON report to {out_file}: {e}"))?;

    Ok(all_passed)
}
