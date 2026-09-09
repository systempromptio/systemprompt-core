//! Typed case, rubric and dataset content stored as immutable revisions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::EvalRevisionId;

use super::invalid;
use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Partition {
    Development,
    Holdout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseContent {
    pub prompt: String,
    pub expected_behavior: Vec<String>,
    pub fixtures: BTreeMap<String, String>,
    pub partition: Partition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedDimension {
    pub name: String,
    pub description: String,
    pub weight: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RubricContent {
    pub dimensions: Vec<WeightedDimension>,
    pub pass_threshold_milli: u32,
    pub hard_gates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "content",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ResourceContent {
    Case(CaseContent),
    Rubric(RubricContent),
    Dataset(Vec<EvalRevisionId>),
}

impl ResourceContent {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Case(_) => "case",
            Self::Rubric(_) => "rubric",
            Self::Dataset(_) => "dataset",
        }
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Case(case) => {
                if case.prompt.trim().is_empty() || case.expected_behavior.is_empty() {
                    return Err(invalid("Cases require a prompt and expected behavior"));
                }
                if case.fixtures.keys().any(|path| {
                    path.starts_with('/')
                        || path.contains('\\')
                        || path.split('/').any(|part| matches!(part, ".." | "." | ""))
                }) {
                    return Err(invalid(
                        "Fixture paths must remain inside the execution workspace",
                    ));
                }
            },
            Self::Rubric(rubric) => rubric.validate()?,
            Self::Dataset(cases) if cases.is_empty() => {
                return Err(invalid("Dataset must contain cases"));
            },
            Self::Dataset(cases) => {
                for (index, case) in cases.iter().enumerate() {
                    if cases[..index].contains(case) {
                        return Err(invalid("Dataset contains duplicate cases"));
                    }
                }
            },
        }
        Ok(())
    }
}

impl RubricContent {
    pub fn validate(&self) -> Result<()> {
        if self.dimensions.is_empty()
            || self.dimensions.len() > 20
            || !(1000..=5000).contains(&self.pass_threshold_milli)
        {
            return Err(invalid(
                "Rubric requires 1–20 dimensions and a threshold between 1000 and 5000",
            ));
        }
        for (index, gate) in self.hard_gates.iter().enumerate() {
            if gate.trim().is_empty() || self.hard_gates[..index].contains(gate) {
                return Err(invalid("Hard-gate names must be nonempty and unique"));
            }
        }
        for (index, dimension) in self.dimensions.iter().enumerate() {
            if dimension.name.trim().is_empty()
                || dimension.description.trim().is_empty()
                || !(1..=10_000).contains(&dimension.weight)
                || self.dimensions[..index]
                    .iter()
                    .any(|other| other.name == dimension.name)
            {
                return Err(invalid(
                    "Rubric dimension names must be unique and weights must be 1–10000",
                ));
            }
        }
        Ok(())
    }
}
