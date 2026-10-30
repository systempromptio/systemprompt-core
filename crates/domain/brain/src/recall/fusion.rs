//! Reciprocal rank fusion of the retrieval tiers: a fact scores
//! `Σ weight_tier / (k + rank)` over every tier that returned it (rank
//! counted from 1), and facts are ordered by score with ties broken by
//! fact id. Extension retrievers only re-rank facts a core tier already
//! returned, so an extension cannot surface a fact the scope-filtered SQL
//! did not admit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{BrainFact, RetrievalTier, ScoredFact, TierRank, TierWeights};

/// The facts one tier returned, in rank order.
#[derive(Debug, Clone, PartialEq)]
pub struct TierHits {
    pub tier: RetrievalTier,
    pub facts: Vec<BrainFact>,
}

impl TierHits {
    #[must_use]
    pub const fn new(tier: RetrievalTier, facts: Vec<BrainFact>) -> Self {
        Self { tier, facts }
    }
}

#[must_use]
pub const fn tier_weight(weights: &TierWeights, tier: RetrievalTier) -> f64 {
    match tier {
        RetrievalTier::Pinned => weights.pinned,
        RetrievalTier::Subject => weights.subject,
        RetrievalTier::Context => weights.context,
        RetrievalTier::FullText => weights.full_text,
        RetrievalTier::Extension => weights.extension,
    }
}

fn contribution(weights: &TierWeights, k: u32, tier: RetrievalTier, rank: u32) -> f64 {
    tier_weight(weights, tier) / (f64::from(k) + f64::from(rank))
}

#[must_use]
pub fn fuse(tiers: Vec<TierHits>, weights: &TierWeights, k: u32) -> Vec<ScoredFact> {
    let mut fused: Vec<ScoredFact> = Vec::new();
    let mut index: HashMap<BrainFactId, usize> = HashMap::new();
    for hits in tiers {
        for (position, fact) in hits.facts.into_iter().enumerate() {
            let rank = u32::try_from(position + 1).unwrap_or(u32::MAX);
            let score = contribution(weights, k, hits.tier, rank);
            let tier_rank = TierRank {
                tier: hits.tier,
                rank,
            };
            if let Some(&at) = index.get(&fact.id) {
                fused[at].score += score;
                fused[at].ranks.push(tier_rank);
            } else {
                index.insert(fact.id.clone(), fused.len());
                fused.push(ScoredFact {
                    fact,
                    score,
                    ranks: vec![tier_rank],
                });
            }
        }
    }
    sort_ranked(&mut fused);
    fused
}

pub fn boost(fused: &mut [ScoredFact], ranked_ids: &[BrainFactId], weights: &TierWeights, k: u32) {
    for (position, id) in ranked_ids.iter().enumerate() {
        let rank = u32::try_from(position + 1).unwrap_or(u32::MAX);
        if let Some(scored) = fused.iter_mut().find(|scored| &scored.fact.id == id) {
            scored.score += contribution(weights, k, RetrievalTier::Extension, rank);
            scored.ranks.push(TierRank {
                tier: RetrievalTier::Extension,
                rank,
            });
        }
    }
    sort_ranked(fused);
}

pub fn sort_ranked(fused: &mut [ScoredFact]) {
    fused.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.fact.id.cmp(&b.fact.id))
    });
}

#[must_use]
pub fn found_by(scored: &ScoredFact, tier: RetrievalTier) -> bool {
    scored.ranks.iter().any(|rank| rank.tier == tier)
}
