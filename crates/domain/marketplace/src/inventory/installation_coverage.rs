//! Installation acknowledgment coverage is cached separately from invocation
//! attribution.
//!
//! The cache is a projection: the marketplace schema's statement triggers
//! queue an owner whenever its receipts, grants, publication selections or
//! consumers' devices change, and the `managed_inventory_refresh` job drains
//! the queue through `refresh_managed_installation_coverage`. A rebuild that
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
