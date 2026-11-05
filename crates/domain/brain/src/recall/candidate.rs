//! A fill candidate and the arithmetic the fill shares.
//!
//! The token estimate (a quarter of the line's characters, rounded up), the
//! usage term (`usage_weight × (cited_rate − ignored_rate)`), the share-of-max
//! slot count, the core-tier tests (context-only, extension-only) and the
//! [`FillOrder`] sort.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::brain::{BrainFact, FillOrder, RetrievalTier, ScoredFact};

/// A visible fact, the text it would be injected as, and its in-context
/// importance.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub scored: ScoredFact,
    pub text: String,
    pub importance: f64,
}

#[must_use]
pub fn estimate_tokens(text: &str) -> u32 {
    u32::try_from(text.chars().count().div_ceil(4)).unwrap_or(u32::MAX)
}

pub(super) fn share_of(max_facts: u32, share: f64, round: fn(f64) -> f64) -> u32 {
    // Why: lint-ok: a slot count at most max_facts; the rounding is exact.
    let slots = round(f64::from(max_facts) * share.clamp(0.0, 1.0)) as u32;
    slots.min(max_facts)
}

fn core_ranks(scored: &ScoredFact) -> impl Iterator<Item = RetrievalTier> + '_ {
    scored
        .ranks
        .iter()
        .map(|rank| rank.tier)
        .filter(|tier| *tier != RetrievalTier::Extension)
}

pub(super) fn context_only(scored: &ScoredFact) -> bool {
    core_ranks(scored).next().is_some() && core_ranks(scored).all(|t| t == RetrievalTier::Context)
}

pub(super) fn extension_only(scored: &ScoredFact) -> bool {
    !scored.ranks.is_empty() && core_ranks(scored).next().is_none()
}

#[must_use]
pub fn usage_term(fact: &BrainFact, usage_weight: f64) -> f64 {
    usage_weight.mul_add(fact.usage_cited_rate - fact.usage_ignored_rate, 0.0)
}

pub(super) fn apply_usage(candidates: &mut [Candidate], usage_weight: f64) {
    for candidate in candidates {
        let adjusted = candidate.scored.score + usage_term(&candidate.scored.fact, usage_weight);
        candidate.scored.score = adjusted.max(0.0);
    }
}

pub(super) fn sort_candidates(candidates: &mut [Candidate], order: FillOrder, usage_weight: f64) {
    candidates.sort_by(|a, b| {
        let usage = |c: &Candidate| usage_term(&c.scored.fact, usage_weight);
        let (primary, secondary) = match order {
            FillOrder::Rank => (
                b.scored.score.total_cmp(&a.scored.score),
                b.importance.total_cmp(&a.importance),
            ),
            FillOrder::Importance => (
                b.importance.total_cmp(&a.importance),
                b.scored.score.total_cmp(&a.scored.score),
            ),
        };
        primary
            .then(secondary)
            .then_with(|| usage(b).total_cmp(&usage(a)))
            .then_with(|| a.scored.fact.id.cmp(&b.scored.fact.id))
    });
}
