//! Rubric model describing judge scoring criteria.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::EvalRubricId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RubricDimension {
    pub name: String,
    pub description: String,
    #[serde(default = "default_weight")]
    pub weight: f64,
}

const fn default_weight() -> f64 {
    1.0
}

#[derive(Debug, Clone)]
pub struct Rubric {
    pub id: EvalRubricId,
    pub name: String,
    pub dimensions: Vec<RubricDimension>,
    pub pass_threshold: i32,
    pub prompt_template: Option<String>,
    pub enabled: bool,
}

/// Structured output the judge model is constrained to produce.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeVerdict {
    pub overall_score: i32,
    #[serde(default)]
    pub dimension_scores: Vec<DimensionScore>,
    pub rationale: String,
    #[serde(default)]
    pub repair_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionScore {
    pub name: String,
    pub score: i32,
}

impl JudgeVerdict {
    /// JSON schema the judge request is constrained with; keep in sync with
    /// the `Deserialize` shape above.
    #[must_use]
    pub fn response_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "overall_score": { "type": "integer", "minimum": 1, "maximum": 5 },
                "dimension_scores": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "score": { "type": "integer", "minimum": 1, "maximum": 5 }
                        },
                        "required": ["name", "score"]
                    }
                },
                "rationale": { "type": "string" },
                "repair_hint": { "type": ["string", "null"] }
            },
            "required": ["overall_score", "rationale"]
        })
    }
}
