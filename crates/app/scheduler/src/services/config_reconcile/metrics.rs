//! Prometheus gauges for declared configuration planes.
//!
//! `systemprompt_config_info{plane,hash,authority}` is 1 for the declaration
//! hash a plane last reconciled against; when the hash moves, the previous
//! series is set to 0 so a dashboard keyed on `== 1` sees exactly one hash per
//! plane. The gauges live on the replica that ran the pass.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::PlaneReport;

pub const CONFIG_DRIFT_ROWS: &str = "systemprompt_config_drift_rows";
pub const CONFIG_INFO: &str = "systemprompt_config_info";

pub(super) fn record(report: &PlaneReport, previous_hash: Option<&str>) {
    metrics::gauge!(CONFIG_DRIFT_ROWS, "plane" => report.plane.clone())
        .set(report.drift_rows as f64);
    let authority = report.authority.as_str();
    if let Some(previous) = previous_hash
        && report.declared_hash.as_deref() != Some(previous)
    {
        metrics::gauge!(
            CONFIG_INFO,
            "plane" => report.plane.clone(),
            "hash" => previous.to_owned(),
            "authority" => authority,
        )
        .set(0.0);
    }
    if let Some(hash) = &report.declared_hash {
        metrics::gauge!(
            CONFIG_INFO,
            "plane" => report.plane.clone(),
            "hash" => hash.clone(),
            "authority" => authority,
        )
        .set(1.0);
    }
}
