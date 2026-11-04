//! Reciprocal rank fusion of the retrieval tiers, over fact ids.
//!
//! A fact scores `Σ weight_tier / (k + rank)` over every tier that returned
//! it (rank counted from 1), and facts are ordered by score with ties
//! broken by fact id. The tiers fuse as ids ([`TierIds`]); the fused facts
//! are loaded once afterwards and attached in the fused order by
//! [`attach`]. Extension retrievers are fused as the `Extension` tier, one
//! `TierIds` per retriever (see [`super::introduce`]); a fact only they
//! return reaches the same lifecycle and visibility checks as every other
//! hit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::hash::BuildHasher;

use systemprompt_extension::brain::TierIds;
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{BrainFact, RetrievalTier, ScoredFact, TierRank, TierWeights};

/// A fused fact id with its score and the tier ranks behind it.
#[derive(Debug, Clone, PartialEq)]
pub struct RankedId {
    pub id: BrainFactId,
    pub score: f64,
    pub ranks: Vec<TierRank>,
}

#[must_use]
pub const fn tier_weight(weights: &TierWeights, tier: RetrievalTier) -> f64 {
    match tier {
        RetrievalTier::Pinned => weights.pinned,
        RetrievalTier::Subject => weights.subject,
        RetrievalTier::Linked => weights.linked,
        RetrievalTier::Context => weights.context,
        RetrievalTier::FullText => weights.full_text,
        RetrievalTier::Extension => weights.extension,
    }
}

fn contribution(weights: &TierWeights, k: u32, tier: RetrievalTier, rank: u32) -> f64 {
    tier_weight(weights, tier) / (f64::from(k) + f64::from(rank))
}

#[must_use]
pub fn fuse(tiers: Vec<TierIds>, weights: &TierWeights, k: u32) -> Vec<RankedId> {
    let mut fused: Vec<RankedId> = Vec::new();
    let mut index: HashMap<BrainFactId, usize> = HashMap::new();
    for hits in tiers {
        for (position, id) in hits.ids.into_iter().enumerate() {
            let rank = u32::try_from(position + 1).unwrap_or(u32::MAX);
            let score = contribution(weights, k, hits.tier, rank);
            let tier_rank = TierRank {
                tier: hits.tier,
                rank,
            };
            if let Some(&at) = index.get(&id) {
                fused[at].score += score;
                fused[at].ranks.push(tier_rank);
            } else {
                index.insert(id.clone(), fused.len());
                fused.push(RankedId {
                    id,
                    score,
                    ranks: vec![tier_rank],
                });
            }
        }
    }
    fused.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
    fused
}

#[must_use]
pub fn attach<S: BuildHasher>(
    ranked: Vec<RankedId>,
    mut facts: HashMap<BrainFactId, BrainFact, S>,
) -> Vec<ScoredFact> {
    ranked
        .into_iter()
        .filter_map(|ranked| {
            facts.remove(&ranked.id).map(|fact| ScoredFact {
                fact,
                score: ranked.score,
                ranks: ranked.ranks,
            })
        })
        .collect()
}

#[must_use]
pub fn found_by(scored: &ScoredFact, tier: RetrievalTier) -> bool {
    scored.ranks.iter().any(|rank| rank.tier == tier)
}
