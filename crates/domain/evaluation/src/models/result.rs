//! Per-case evaluation result model.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use systemprompt_identifiers::{AiRequestId, EvalCaseId, EvalResultId, EvalRunId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Pass,
    Partial,
    Fail,
    Skipped,
}

impl Verdict {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Partial => "partial",
            Self::Fail => "fail",
            Self::Skipped => "skipped",
        }
    }
}

impl std::str::FromStr for Verdict {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pass" => Ok(Self::Pass),
            "partial" => Ok(Self::Partial),
            "fail" => Ok(Self::Fail),
            "skipped" => Ok(Self::Skipped),
            other => Err(format!("unknown verdict: {other}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EvalResult {
    pub id: EvalResultId,
    pub run_id: EvalRunId,
    pub ai_request_id: Option<AiRequestId>,
    pub case_id: Option<EvalCaseId>,
    pub provider: String,
    pub model: String,
    pub overall_score: Option<i32>,
    pub dimension_scores: Value,
    pub verdict: Verdict,
    pub rationale: Option<String>,
    pub repair_hint: Option<String>,
    pub prompt_excerpt: Option<String>,
    pub response_excerpt: Option<String>,
    pub latency_ms: Option<i32>,
    pub cost_microdollars: i64,
    pub judge_cost_microdollars: i64,
    pub created_at: DateTime<Utc>,
    pub repaired: bool,
    pub replay_of_result_id: Option<EvalResultId>,
    pub judge_ai_request_id: Option<AiRequestId>,
}

#[derive(Debug, Clone)]
pub struct NewResultParams {
    pub run_id: EvalRunId,
    pub ai_request_id: Option<AiRequestId>,
    pub case_id: Option<EvalCaseId>,
    pub provider: String,
    pub model: String,
    pub overall_score: Option<i32>,
    pub dimension_scores: Value,
    pub verdict: Verdict,
    pub rationale: Option<String>,
    pub repair_hint: Option<String>,
    pub prompt_excerpt: Option<String>,
    pub response_excerpt: Option<String>,
    pub judge_cost_microdollars: i64,
    pub repaired: bool,
    pub replay_of_result_id: Option<EvalResultId>,
    pub judge_ai_request_id: Option<AiRequestId>,
}
