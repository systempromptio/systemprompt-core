//! Analyses over measured history that draft `brain_proposals`; nothing an
//! analysis drafts applies itself.
//!
//! The core analyses are [`BloatAnalysis`] (live facts per kind against
//! injections), [`RelevanceAnalysis`] (injections against citations per
//! kind) and [`CoverageAnalysis`] (recalls that injected nothing, per
//! client); extensions add their own through the `BrainAnalysis` seam.
//! [`BrainAnalysisJob`] runs them daily and stores each draft unless an open
//! proposal from the same analysis already carries the same body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod bloat;
pub mod coverage;
mod job;
pub mod relevance;
pub mod store;

pub use bloat::{BLOAT_ANALYSIS, BloatAction, BloatAnalysis, BloatDiff, BloatThresholds};
pub use coverage::{COVERAGE_ANALYSIS, CoverageAnalysis, CoverageNote, CoverageThresholds};
pub use job::BrainAnalysisJob;
pub use relevance::{
    RELEVANCE_ANALYSIS, RelevanceAnalysis, RelevanceDiff, RelevanceThresholds, WeightAdjustment,
};
pub use store::{AnalysisRepository, ClientRecalls, KindUsage};

use systemprompt_models::brain::{BrainProposal, ProposalDraft};

#[must_use]
pub fn same_proposal(draft: &ProposalDraft, open: &BrainProposal) -> bool {
    draft.analysis == open.analysis && draft.kind == open.kind && draft.body == open.body
}

#[must_use]
pub fn fresh_drafts(drafts: Vec<ProposalDraft>, open: &[BrainProposal]) -> Vec<ProposalDraft> {
    let mut fresh: Vec<ProposalDraft> = Vec::with_capacity(drafts.len());
    for draft in drafts {
        let stored = open.iter().any(|o| same_proposal(&draft, o));
        let repeated = fresh
            .iter()
            .any(|f| f.analysis == draft.analysis && f.kind == draft.kind && f.body == draft.body);
        if !stored && !repeated {
            fresh.push(draft);
        }
    }
    fresh
}
