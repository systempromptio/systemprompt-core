use std::collections::BTreeMap;

use serde::Serialize;

use crate::config::Thresholds;
use crate::metrics::{Report, ScenarioJson};

#[derive(Serialize)]
struct Aggregate {
    total_requests: u64,
    all_passed: bool,
}

#[derive(Serialize)]
struct JsonReport {
    scenarios: BTreeMap<String, ScenarioJson>,
    aggregate: Aggregate,
}

pub fn write(report: &Report, thresholds: &Thresholds, out_file: &str) -> Result<bool, String> {
    let mut scenarios = BTreeMap::new();
    let mut total_requests = 0u64;
    let mut all_passed = true;

    for (name, snapshot) in &report.scenarios {
        let record = snapshot.to_json(thresholds);
        total_requests += record.requests;
        if !record.passed {
            all_passed = false;
        }
        scenarios.insert(name.clone(), record);
    }

    let json_report = JsonReport {
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
