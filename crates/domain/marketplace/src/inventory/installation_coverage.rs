//! Installation acknowledgment coverage is cached separately from invocation
//! attribution.
//!
//! The cache is recomputed by the `managed_inventory_refresh` job, which reads
//! back how many resources' coverage changed. A pass that
//! finds every resource's coverage unchanged writes nothing: the generation
//! and `observed_at` on `managed_installation_coverage_state` move only when
//! at least one resource's body changed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// Current eligible enrolled-device coverage and retained authenticated
/// installation evidence.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct InstallationCoverage {
    pub eligible_devices: i64,
    pub current_acknowledged_devices: i64,
    pub current_verified_devices: i64,
    pub acknowledged_installations: i64,
    pub unverifiable_installations: i64,
    pub legacy_receipts: i64,
}
