//! [`BloatAnalysis`]: a kind that holds many live facts recall almost never
//! injects costs extraction and storage for nothing. It drafts a config
//! diff that retires the kind when nothing was injected, else raises its
//! materiality bar.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_extension::brain::{AnalysisWindow, BrainAnalysis, BrainAnalysisError};
use systemprompt_identifiers::FactKindId;
use systemprompt_models::brain::{ProposalDraft, ProposalKind};

use super::store::{AnalysisRepository, KindUsage};

pub const BLOAT_ANALYSIS: &str = "bloat";

/// When a kind counts as bloated: at least `min_facts` live facts and at
/// most one injection per `facts_per_injection` facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BloatThresholds {
    pub min_facts: i64,
    pub facts_per_injection: i64,
}

impl Default for BloatThresholds {
    fn default() -> Self {
        Self {
            min_facts: 50,
            facts_per_injection: 100,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BloatAction {
    RetireKind,
    RaiseMateriality,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BloatDiff {
    pub kind: FactKindId,
    pub facts: i64,
    pub injections: i64,
    pub action: BloatAction,
}

#[must_use]
pub fn bloat_decision(usage: &KindUsage, thresholds: BloatThresholds) -> Option<BloatAction> {
    if usage.facts < thresholds.min_facts {
        return None;
    }
    if usage.injections == 0 {
        return Some(BloatAction::RetireKind);
    }
    let per_injection = thresholds.facts_per_injection.max(1);
    (usage.injections.saturating_mul(per_injection) <= usage.facts)
        .then_some(BloatAction::RaiseMateriality)
}

pub fn bloat_drafts(
    usage: &[KindUsage],
    thresholds: BloatThresholds,
) -> Result<Vec<ProposalDraft>, serde_json::Error> {
    usage
        .iter()
        .filter_map(|u| bloat_decision(u, thresholds).map(|action| (u, action)))
        .map(|(u, action)| {
            let diff = BloatDiff {
                kind: u.kind.clone(),
                facts: u.facts,
                injections: u.injections,
                action,
            };
            Ok(ProposalDraft {
                analysis: BLOAT_ANALYSIS.to_owned(),
                kind: ProposalKind::ConfigDiff,
                summary: format!(
                    "Kind {} holds {} live facts but was injected {} times",
                    u.kind, u.facts, u.injections
                ),
                // JSON: the proposal body column stores the typed diff.
                body: serde_json::to_value(diff)?,
            })
        })
        .collect()
}

/// Drafts bloat proposals from the kind usage aggregate.
#[derive(Debug, Clone)]
pub struct BloatAnalysis {
    store: AnalysisRepository,
    thresholds: BloatThresholds,
}

impl BloatAnalysis {
    #[must_use]
    pub const fn new(store: AnalysisRepository, thresholds: BloatThresholds) -> Self {
        Self { store, thresholds }
    }
}

#[async_trait::async_trait]
impl BrainAnalysis for BloatAnalysis {
    fn name(&self) -> &'static str {
        BLOAT_ANALYSIS
    }

    async fn analyse(
        &self,
        window: AnalysisWindow,
    ) -> Result<Vec<ProposalDraft>, BrainAnalysisError> {
        let usage = self.store.kind_usage(window).await?;
        bloat_drafts(&usage, self.thresholds).map_err(|e| BrainAnalysisError::Other {
            analysis: BLOAT_ANALYSIS,
            source: Box::new(e),
        })
    }
}
