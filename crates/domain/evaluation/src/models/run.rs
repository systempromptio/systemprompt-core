//! Evaluation run model and lifecycle states.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{EvalRubricId, EvalRunId, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvalRunKind {
    Judge,
    Replay,
    Pairwise,
}

impl EvalRunKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Judge => "judge",
            Self::Replay => "replay",
            Self::Pairwise => "pairwise",
        }
    }
}

impl std::str::FromStr for EvalRunKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "judge" => Ok(Self::Judge),
            "replay" => Ok(Self::Replay),
            "pairwise" => Ok(Self::Pairwise),
            other => Err(format!("unknown eval run kind: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvalRunStatus {
    Running,
    Completed,
    Failed,
}

impl EvalRunStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TriggerSource {
    Scheduled,
    Cli,
    Manual,
}

impl TriggerSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Cli => "cli",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EvalRun {
    pub id: EvalRunId,
    pub kind: EvalRunKind,
    pub status: EvalRunStatus,
    pub judge_provider: String,
    pub judge_model: String,
    pub sample_size: i32,
    pub scored_count: i32,
    pub failed_count: i32,
    pub cost_microdollars: i64,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub rubric_id: Option<EvalRubricId>,
    pub trigger_source: TriggerSource,
}

#[derive(Debug, Clone)]
pub struct NewRunParams {
    pub kind: EvalRunKind,
    pub judge_provider: String,
    pub judge_model: String,
    pub sample_size: i32,
    pub created_by: UserId,
    pub rubric_id: Option<EvalRubricId>,
    pub trigger_source: TriggerSource,
}
