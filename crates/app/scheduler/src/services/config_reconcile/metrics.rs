//! Prometheus gauges for declared configuration planes.
//!
//! `systemprompt_config_plane_info{plane,hash,authority}` is 1 for the
//! declaration hash and authority a plane last reconciled under. When either
//! moves, the series the previous pass recorded (its own hash and authority
//! labels, read from `config_plane_state`) is set to 0, so a dashboard keyed
//! on `== 1` sees exactly one series per plane. The `metrics` facade cannot
//! unregister a series; a zeroed one stays until the process restarts. The
//! gauges live on the replica that ran the pass.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::PlaneReport;
use crate::repository::PlaneStateRow;

pub const CONFIG_DRIFT_ROWS: &str = "systemprompt_config_drift_rows";
pub const CONFIG_PLANE_INFO: &str = "systemprompt_config_plane_info";

pub(super) fn record(report: &PlaneReport, previous: Option<&PlaneStateRow>) {
    metrics::gauge!(CONFIG_DRIFT_ROWS, "plane" => report.plane.clone())
        .set(report.drift_rows as f64);
    let authority = report.authority.as_str();
    if let Some(previous) = previous
        && let Some(previous_hash) = previous.declared_hash.as_deref()
        && (report.declared_hash.as_deref() != Some(previous_hash)
            || previous.authority != authority)
    {
        metrics::gauge!(
            CONFIG_PLANE_INFO,
            "plane" => report.plane.clone(),
            "hash" => previous_hash.to_owned(),
            "authority" => previous.authority.clone(),
        )
        .set(0.0);
    }
    if let Some(hash) = &report.declared_hash {
        metrics::gauge!(
            CONFIG_PLANE_INFO,
            "plane" => report.plane.clone(),
            "hash" => hash.clone(),
            "authority" => authority,
        )
        .set(1.0);
    }
}
