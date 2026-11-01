//! [`RelevanceAnalysis`].
//!
//! How often an injected fact of a kind was cited, per workflow the recall
//! selected. A kind cited rarely under a workflow gets a lower importance
//! weight there, one cited often a higher one, as one config diff over
//! `importance.yaml`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_extension::brain::{AnalysisWindow, BrainAnalysis, BrainAnalysisError};
use systemprompt_identifiers::{FactKindId, WorkflowId};
use systemprompt_models::brain::{ProposalDraft, ProposalKind};

use super::store::{AnalysisRepository, WorkflowKindUsage};

pub const RELEVANCE_ANALYSIS: &str = "relevance";
pub const IMPORTANCE_DOC: &str = "importance.yaml";

/// Citation rates outside `low..=high`, over at least `min_injections`
/// injections, move a kind's weight by `step`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelevanceThresholds {
    pub min_injections: i64,
    pub low: f64,
    pub high: f64,
    pub step: f64,
}

impl Default for RelevanceThresholds {
    fn default() -> Self {
        Self {
            min_injections: 20,
            low: 0.05,
            high: 0.5,
            step: 0.5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightAdjustment {
    pub workflow: Option<WorkflowId>,
    pub kind: FactKindId,
    pub injected: i64,
    pub cited: i64,
    pub cite_rate: f64,
    pub weight_factor: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelevanceDiff {
    pub target: String,
    pub adjustments: Vec<WeightAdjustment>,
}

#[must_use]
pub fn weight_adjustment(
    usage: &WorkflowKindUsage,
    thresholds: RelevanceThresholds,
) -> Option<WeightAdjustment> {
    if usage.injections < thresholds.min_injections.max(1) {
        return None;
    }
    #[expect(clippy::cast_precision_loss, reason = "counts far below 2^52")]
    let cite_rate = usage.citations as f64 / usage.injections as f64;
    let weight_factor = if cite_rate < thresholds.low {
        1.0 - thresholds.step
    } else if cite_rate > thresholds.high {
        1.0 + thresholds.step
    } else {
        return None;
    };
    Some(WeightAdjustment {
        workflow: usage.workflow.clone(),
        kind: usage.kind.clone(),
        injected: usage.injections,
        cited: usage.citations,
        cite_rate,
        weight_factor,
    })
}

pub fn relevance_drafts(
    usage: &[WorkflowKindUsage],
    thresholds: RelevanceThresholds,
) -> Result<Vec<ProposalDraft>, serde_json::Error> {
    let adjustments: Vec<WeightAdjustment> = usage
        .iter()
        .filter_map(|u| weight_adjustment(u, thresholds))
        .collect();
    if adjustments.is_empty() {
        return Ok(Vec::new());
    }
    let summary = format!(
        "Adjust {} importance weights by citation rate per workflow and kind",
        adjustments.len()
    );
    let diff = RelevanceDiff {
        target: IMPORTANCE_DOC.to_owned(),
        adjustments,
    };
    Ok(vec![ProposalDraft {
        analysis: RELEVANCE_ANALYSIS.to_owned(),
        kind: ProposalKind::ConfigDiff,
        summary,
        // JSON: the proposal body column stores the typed diff.
        body: serde_json::to_value(diff)?,
    }])
}

/// Drafts weight adjustments from injected versus cited counts.
#[derive(Debug, Clone)]
pub struct RelevanceAnalysis {
    store: AnalysisRepository,
    thresholds: RelevanceThresholds,
}

impl RelevanceAnalysis {
    #[must_use]
    pub const fn new(store: AnalysisRepository, thresholds: RelevanceThresholds) -> Self {
        Self { store, thresholds }
    }
}

#[async_trait::async_trait]
impl BrainAnalysis for RelevanceAnalysis {
    fn name(&self) -> &'static str {
        RELEVANCE_ANALYSIS
    }

    async fn analyse(
        &self,
        window: AnalysisWindow,
    ) -> Result<Vec<ProposalDraft>, BrainAnalysisError> {
        let usage = self.store.workflow_kind_usage(window).await?;
        relevance_drafts(&usage, self.thresholds).map_err(|e| BrainAnalysisError::Other {
            analysis: RELEVANCE_ANALYSIS,
            source: Box::new(e),
        })
    }
}
