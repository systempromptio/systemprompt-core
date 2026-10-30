use systemprompt_brain::recall::fusion::{TierHits, boost, found_by, fuse};
use systemprompt_models::brain::{RetrievalTier, TierWeights};

use crate::recall_fixtures::{brain_fact, id};

const K: u32 = 60;

fn equal_weights() -> TierWeights {
    TierWeights {
        pinned: 1.0,
        subject: 1.0,
        context: 1.0,
        full_text: 1.0,
        extension: 1.0,
    }
}

#[test]
fn score_is_weight_over_k_plus_one_based_rank() {
    let fused = fuse(
        vec![TierHits::new(
            RetrievalTier::FullText,
            vec![brain_fact(1, "a"), brain_fact(2, "a")],
        )],
        &TierWeights::default(),
        K,
    );
    assert_eq!(fused.len(), 2);
    assert!((fused[0].score - 0.6 / 61.0).abs() < 1e-12);
    assert!((fused[1].score - 0.6 / 62.0).abs() < 1e-12);
}

#[test]
fn a_fact_found_by_two_tiers_outranks_a_better_single_tier_hit() {
    let fused = fuse(
        vec![
            TierHits::new(
                RetrievalTier::Subject,
                vec![brain_fact(1, "a"), brain_fact(2, "a")],
            ),
            TierHits::new(RetrievalTier::FullText, vec![brain_fact(2, "a")]),
        ],
        &equal_weights(),
        K,
    );
    assert_eq!(fused[0].fact.id, id(2));
    assert!((fused[0].score - (1.0 / 62.0 + 1.0 / 61.0)).abs() < 1e-12);
    assert!(found_by(&fused[0], RetrievalTier::Subject));
    assert!(found_by(&fused[0], RetrievalTier::FullText));
    assert!(!found_by(&fused[1], RetrievalTier::FullText));
}

#[test]
fn equal_scores_are_ordered_by_fact_id() {
    let fused = fuse(
        vec![
            TierHits::new(RetrievalTier::Subject, vec![brain_fact(9, "a")]),
            TierHits::new(RetrievalTier::Pinned, vec![brain_fact(3, "a")]),
            TierHits::new(RetrievalTier::FullText, vec![brain_fact(5, "a")]),
        ],
        &equal_weights(),
        K,
    );
    let ids: Vec<_> = fused.iter().map(|s| s.fact.id.clone()).collect();
    assert_eq!(ids, vec![id(3), id(5), id(9)]);
}

#[test]
fn a_zero_weight_tier_adds_nothing() {
    let mut weights = equal_weights();
    weights.context = 0.0;
    let fused = fuse(
        vec![
            TierHits::new(RetrievalTier::Context, vec![brain_fact(1, "a")]),
            TierHits::new(RetrievalTier::Subject, vec![brain_fact(2, "a")]),
        ],
        &weights,
        K,
    );
    assert_eq!(fused[0].fact.id, id(2));
    assert!(fused[1].score.abs() < 1e-12);
}

#[test]
fn extension_ranking_reorders_known_facts_and_ignores_unknown_ids() {
    let mut fused = fuse(
        vec![TierHits::new(
            RetrievalTier::FullText,
            vec![brain_fact(1, "a"), brain_fact(2, "a")],
        )],
        &equal_weights(),
        K,
    );
    boost(&mut fused, &[id(7), id(2)], &equal_weights(), K);
    assert_eq!(fused.len(), 2);
    assert_eq!(fused[0].fact.id, id(2));
    assert!(found_by(&fused[0], RetrievalTier::Extension));
    assert!(!found_by(&fused[1], RetrievalTier::Extension));
}
