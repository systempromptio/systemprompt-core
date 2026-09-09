//! Deterministic weighted scoring requires a complete dimension and evidence
//! set.

use super::invalid;
use super::resources::RubricContent;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceScore {
    pub name: String,
    pub score: u32,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceJudgment {
    pub dimensions: Vec<EvidenceScore>,
    pub hard_gates: BTreeMap<String, bool>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WeightedOutcome {
    pub score_milli: u32,
    pub passed: bool,
}

pub fn score(
    rubric: &RubricContent,
    judgment: &EvidenceJudgment,
    evidence: &BTreeSet<String>,
) -> Result<WeightedOutcome> {
    rubric.validate()?;
    if judgment.dimensions.len() != rubric.dimensions.len() || judgment.rationale.trim().is_empty()
    {
        return Err(invalid("Judgment requires every dimension and a rationale"));
    }
    let mut total = 0_u64;
    let mut weights = 0_u64;
    for dimension in &rubric.dimensions {
        let matches = judgment
            .dimensions
            .iter()
            .filter(|score| score.name == dimension.name)
            .collect::<Vec<_>>();
        let [score] = matches.as_slice() else {
            return Err(invalid("Missing or duplicate dimension score"));
        };
        if !(1..=5).contains(&score.score)
            || score.evidence.is_empty()
            || score
                .evidence
                .iter()
                .any(|reference| !evidence.contains(reference))
        {
            return Err(invalid(
                "Dimension scores require a 1–5 score and resolvable evidence",
            ));
        }
        total += u64::from(score.score) * u64::from(dimension.weight) * 1000;
        weights += u64::from(dimension.weight);
    }
    if judgment.hard_gates.len() != rubric.hard_gates.len()
        || rubric
            .hard_gates
            .iter()
            .any(|name| !judgment.hard_gates.contains_key(name))
    {
        return Err(invalid("Judgment requires the exact hard-gate set"));
    }
    let score_milli =
        u32::try_from(total / weights).map_err(|_| invalid("Weighted score overflow"))?;
    Ok(WeightedOutcome {
        score_milli,
        passed: total >= u64::from(rubric.pass_threshold_milli) * weights
            && judgment.hard_gates.values().all(|passed| *passed),
    })
}
