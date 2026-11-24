//! What one reconcile pass, diff or apply produced.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use serde::{Serialize, Serializer};
use systemprompt_provider_contracts::{ApplyOutcome, DriftRow, PlaneEffect};
use systemprompt_runtime::EffectiveAuthority;

use crate::error::SchedulerError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconcileTrigger {
    Boot,
    Scheduled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaneOutcome {
    InSync,
    DriftReported,
    Undeclared,
    Unavailable,
    Failed,
}

impl PlaneOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InSync => "in_sync",
            Self::DriftReported => "drift_reported",
            Self::Undeclared => "undeclared",
            Self::Unavailable => "unavailable",
            Self::Failed => "failed",
        }
    }
}

/// Why a plane was not reconciled: a requirement the host does not provide
/// (the plane is [`PlaneOutcome::Unavailable`]) or the error its read failed
/// with ([`PlaneOutcome::Failed`]). It is rendered only where it leaves the
/// process: the CLI, the HTTP probe and `config_plane_state.last_error`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum PlaneFailure {
    UnmetRequirement {
        requirement: &'static str,
    },
    Failed {
        #[serde(serialize_with = "display")]
        error: Arc<SchedulerError>,
    },
}

impl PlaneFailure {
    #[must_use]
    pub const fn outcome(&self) -> PlaneOutcome {
        match self {
            Self::UnmetRequirement { .. } => PlaneOutcome::Unavailable,
            Self::Failed { .. } => PlaneOutcome::Failed,
        }
    }
}

impl fmt::Display for PlaneFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmetRequirement { requirement } => {
                write!(
                    f,
                    "requires {requirement}, which this host does not provide"
                )
            },
            Self::Failed { error } => error.fmt(f),
        }
    }
}

fn display<S: Serializer>(error: &Arc<SchedulerError>, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(error)
}

/// One plane's result in a reconcile pass.
///
/// `drift_rows` is how many entities differ between the declaration and the
/// database; the pass reports them and writes nothing.
#[derive(Debug, Clone, Serialize)]
pub struct PlaneReport {
    pub plane: String,
    pub authority: EffectiveAuthority,
    pub declared_hash: Option<String>,
    pub drift_rows: usize,
    pub outcome: PlaneOutcome,
    pub failure: Option<PlaneFailure>,
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
    pub fn drift_rows(&self) -> usize {
        self.planes.iter().map(|p| p.drift_rows).sum()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaneDiff {
    pub plane: String,
    pub authority: EffectiveAuthority,
    pub effect: PlaneEffect,
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
