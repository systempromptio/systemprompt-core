//! What one reconcile pass, diff or apply produced.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_provider_contracts::{ApplyOutcome, DriftRow};
use systemprompt_runtime::EffectiveAuthority;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconcileTrigger {
    Boot,
    Scheduled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaneOutcome {
    InSync,
    Corrected,
    Seeded,
    DriftReported,
    Undeclared,
    Failed,
}

impl PlaneOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InSync => "in_sync",
            Self::Corrected => "corrected",
            Self::Seeded => "seeded",
            Self::DriftReported => "drift_reported",
            Self::Undeclared => "undeclared",
            Self::Failed => "failed",
        }
    }
}

/// One plane's result in a reconcile pass.
///
/// `drift_rows` is the drift left after the pass: zero for a corrected or
/// seeded plane, the reported count for a `db` plane.
#[derive(Debug, Clone, Serialize)]
pub struct PlaneReport {
    pub plane: String,
    pub authority: EffectiveAuthority,
    pub declared_hash: Option<String>,
    pub drift_rows: usize,
    pub changes: usize,
    pub outcome: PlaneOutcome,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ReconcileReport {
    pub planes: Vec<PlaneReport>,
}

impl ReconcileReport {
    #[must_use]
    pub fn failures(&self) -> Vec<&PlaneReport> {
        self.planes
            .iter()
            .filter(|p| p.outcome == PlaneOutcome::Failed)
            .collect()
    }

    #[must_use]
    pub fn changes(&self) -> usize {
        self.planes.iter().map(|p| p.changes).sum()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaneDiff {
    pub plane: String,
    pub authority: EffectiveAuthority,
    pub declared_path: String,
    pub declared_hash: Option<String>,
    pub rows: Vec<DriftRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppliedPlane {
    pub plane: String,
    pub declared_hash: String,
    pub outcome: ApplyOutcome,
    pub changes: usize,
    pub remaining_drift: usize,
}
