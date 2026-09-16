//! Paired comparison eligibility uses complete accounting and conservative
//! confidence bounds. Observational production associations are not inputs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::{CampaignPolicy, OptimizationObjective};
use crate::{EvaluationError, Result};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    pub quality_milli: u32,
    pub tokens: u64,
    pub cost_microdollars: u64,
    pub latency_ms: u64,
    pub verified_success: bool,
    pub hard_failures: u32,
    pub accounting_complete: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PairedOutcome {
    pub baseline: Outcome,
    pub candidate: Outcome,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ComparisonDecision {
    pub eligible: bool,
    pub pairs: usize,
    pub mean_improvement: Option<f64>,
    pub lower_confidence_bound: Option<f64>,
    pub reasons: Vec<String>,
}

pub fn compare(policy: &CampaignPolicy, pairs: &[PairedOutcome]) -> Result<ComparisonDecision> {
    policy.validate()?;
    if pairs
        .iter()
        .any(|pair| pair.baseline.quality_milli > 5000 || pair.candidate.quality_milli > 5000)
    {
        return Err(EvaluationError::InvalidSpec(
            "Quality exceeds the five-point scale".to_owned(),
        ));
    }
    let mut reasons = Vec::new();
    if pairs.len() < policy.minimum_pairs as usize {
        reasons.push("Insufficient paired samples".to_owned());
    }
    if pairs
        .iter()
        .any(|pair| !pair.baseline.accounting_complete || !pair.candidate.accounting_complete)
    {
        reasons.push("Incomplete accounting".to_owned());
    }
    if pairs.iter().any(|pair| {
        pair.candidate.hard_failures > 0
            || !pair.candidate.verified_success
            || pair.candidate.quality_milli < policy.minimum_quality_milli
    }) {
        reasons.push("Candidate violates quality or hard gates".to_owned());
    }
    let improvements: Vec<f64> = pairs
        .iter()
        .map(|pair| improvement(policy.objective, pair))
        .collect();
    let (mean, lower) = confidence(&improvements);
    if lower.is_none_or(|bound| bound <= 0.0) {
        reasons.push("Improvement is not established by the paired confidence bound".to_owned());
    }
    let quality: Vec<f64> = pairs
        .iter()
        .map(|pair| {
            f64::from(pair.candidate.quality_milli) - f64::from(pair.baseline.quality_milli)
        })
        .collect();
    if confidence(&quality).1.is_none_or(|bound| bound < 0.0) {
        reasons.push("Quality non-regression is not established".to_owned());
    }
    Ok(ComparisonDecision {
        eligible: reasons.is_empty(),
        pairs: pairs.len(),
        mean_improvement: mean,
        lower_confidence_bound: lower,
        reasons,
    })
}

fn improvement(objective: OptimizationObjective, pair: &PairedOutcome) -> f64 {
    match objective {
        OptimizationObjective::Quality => {
            f64::from(pair.candidate.quality_milli) - f64::from(pair.baseline.quality_milli)
        },
        OptimizationObjective::Tokens => pair.baseline.tokens as f64 - pair.candidate.tokens as f64,
        OptimizationObjective::Cost => {
            pair.baseline.cost_microdollars as f64 - pair.candidate.cost_microdollars as f64
        },
        OptimizationObjective::Latency => {
            pair.baseline.latency_ms as f64 - pair.candidate.latency_ms as f64
        },
    }
}

// Why: repeated runs of one case are not independent statistical evidence.
pub fn collapse_repetitions(pairs: &[PairedOutcome]) -> Result<PairedOutcome> {
    if pairs.is_empty() {
        return Err(EvaluationError::InvalidSpec(
            "A case needs paired measurements".to_owned(),
        ));
    }
    Ok(PairedOutcome {
        baseline: collapse(pairs, |pair| &pair.baseline, false),
        candidate: collapse(pairs, |pair| &pair.candidate, true),
    })
}

fn collapse(
    pairs: &[PairedOutcome],
    project: impl Fn(&PairedOutcome) -> &Outcome,
    candidate: bool,
) -> Outcome {
    let mut result = Outcome {
        quality_milli: if candidate { 5000 } else { 0 },
        tokens: 0,
        cost_microdollars: 0,
        latency_ms: 0,
        verified_success: true,
        hard_failures: 0,
        accounting_complete: true,
    };
    let mut sums = [0u128; 3];
    let mut count = 0u128;
    for value in pairs.iter().map(project) {
        count += 1;
        result.quality_milli = if candidate {
            result.quality_milli.min(value.quality_milli)
        } else {
            result.quality_milli.max(value.quality_milli)
        };
        result.verified_success &= value.verified_success;
        result.accounting_complete &= value.accounting_complete;
        result.hard_failures = result.hard_failures.max(value.hard_failures);
        sums[0] += u128::from(value.tokens);
        sums[1] += u128::from(value.cost_microdollars);
        sums[2] += u128::from(value.latency_ms);
    }
    let average = |sum: u128| {
        if candidate {
            sum.div_ceil(count) as u64
        } else {
            (sum / count) as u64
        }
    };
    result.tokens = average(sums[0]);
    result.cost_microdollars = average(sums[1]);
    result.latency_ms = average(sums[2]);
    result
}

fn confidence(values: &[f64]) -> (Option<f64>, Option<f64>) {
    if values.len() < 2 {
        return (None, None);
    }
    let count = values.len() as f64;
    let mean = values.iter().sum::<f64>() / count;
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (count - 1.0);
    let critical = match values.len() - 1 {
        1 => 12.706,
        2 => 4.303,
        3 => 3.182,
        4 => 2.776,
        5..=9 => 2.571,
        10..=19 => 2.228,
        20..=29 => 2.086,
        _ => 2.045,
    };
    (
        Some(mean),
        Some(mean - critical * (variance / count).sqrt()),
    )
}
