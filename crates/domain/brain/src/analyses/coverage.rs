//! [`CoverageAnalysis`]: recalls that ran and injected nothing are requests
//! the brain could not help. A client where most recalls come back empty
//! gets a fact-actions proposal to add a kind or a capture pathway. Recalls
//! carry no project, so the grouping is by client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_extension::brain::{AnalysisWindow, BrainAnalysis, BrainAnalysisError};
use systemprompt_models::brain::{ProposalDraft, ProposalKind};

use super::store::{AnalysisRepository, ClientRecalls};

pub const COVERAGE_ANALYSIS: &str = "coverage";

/// A client is under-covered with at least `min_recalls` recalls of which
/// at least `empty_share` injected nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoverageThresholds {
    pub min_recalls: i64,
    pub empty_share: f64,
}

impl Default for CoverageThresholds {
    fn default() -> Self {
        Self {
            min_recalls: 20,
            empty_share: 0.5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageAction {
    AddKindOrPathway,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageNote {
    pub client: String,
    pub recalls: i64,
    pub empty: i64,
    pub action: CoverageAction,
}

#[must_use]
pub fn under_covered(recalls: &ClientRecalls, thresholds: CoverageThresholds) -> bool {
    if recalls.recalls < thresholds.min_recalls.max(1) {
        return false;
    }
    #[expect(clippy::cast_precision_loss, reason = "counts far below 2^52")]
    let share = recalls.empty as f64 / recalls.recalls as f64;
    share >= thresholds.empty_share
}

pub fn coverage_drafts(
    clients: &[ClientRecalls],
    thresholds: CoverageThresholds,
) -> Result<Vec<ProposalDraft>, serde_json::Error> {
    clients
        .iter()
        .filter(|c| under_covered(c, thresholds))
        .map(|c| {
            let note = CoverageNote {
                client: c.client.clone(),
                recalls: c.recalls,
                empty: c.empty,
                action: CoverageAction::AddKindOrPathway,
            };
            Ok(ProposalDraft {
                analysis: COVERAGE_ANALYSIS.to_owned(),
                kind: ProposalKind::FactActions,
                summary: format!(
                    "{} of {} recalls from {} injected no fact; add a kind or capture pathway",
                    c.empty, c.recalls, c.client
                ),
                // JSON: the proposal body column stores the typed note.
                body: serde_json::to_value(note)?,
            })
        })
        .collect()
}

/// Drafts coverage-gap notes from recalls that injected nothing.
#[derive(Debug, Clone)]
pub struct CoverageAnalysis {
    store: AnalysisRepository,
    thresholds: CoverageThresholds,
}

impl CoverageAnalysis {
    #[must_use]
    pub const fn new(store: AnalysisRepository, thresholds: CoverageThresholds) -> Self {
        Self { store, thresholds }
    }
}

#[async_trait::async_trait]
impl BrainAnalysis for CoverageAnalysis {
    fn name(&self) -> &'static str {
        COVERAGE_ANALYSIS
    }

    async fn analyse(
        &self,
        window: AnalysisWindow,
    ) -> Result<Vec<ProposalDraft>, BrainAnalysisError> {
        let clients = self.store.client_recalls(window).await?;
        coverage_drafts(&clients, self.thresholds).map_err(|e| BrainAnalysisError::Other {
            analysis: COVERAGE_ANALYSIS,
            source: Box::new(e),
        })
    }
}
