use std::collections::HashMap;

use systemprompt_brain::recall::fusion::{attach, found_by, fuse};
use systemprompt_extension::brain::TierIds;
use systemprompt_models::brain::{RetrievalTier, TierWeights};

use crate::recall_fixtures::{brain_fact, id};

const K: u32 = 60;

fn equal_weights() -> TierWeights {
    TierWeights {
        pinned: 1.0,
        subject: 1.0,
        linked: 1.0,
        context: 1.0,
        full_text: 1.0,
        extension: 1.0,
    }
}

#[test]
fn score_is_weight_over_k_plus_one_based_rank() {
    let fused = fuse(
        vec![TierIds::new(RetrievalTier::FullText, vec![id(1), id(2)])],
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
            TierIds::new(RetrievalTier::Subject, vec![id(1), id(2)]),
            TierIds::new(RetrievalTier::FullText, vec![id(2)]),
        ],
        &equal_weights(),
        K,
    );
    assert_eq!(fused[0].id, id(2));
    assert!((fused[0].score - (1.0 / 62.0 + 1.0 / 61.0)).abs() < 1e-12);
    let facts = HashMap::from([(id(1), brain_fact(1, "a")), (id(2), brain_fact(2, "a"))]);
    let attached = attach(fused, facts);
    assert!(found_by(&attached[0], RetrievalTier::Subject));
    assert!(found_by(&attached[0], RetrievalTier::FullText));
    assert!(!found_by(&attached[1], RetrievalTier::FullText));
}

#[test]
fn equal_scores_are_ordered_by_fact_id() {
    let fused = fuse(
        vec![
            TierIds::new(RetrievalTier::Subject, vec![id(9)]),
            TierIds::new(RetrievalTier::Pinned, vec![id(3)]),
            TierIds::new(RetrievalTier::FullText, vec![id(5)]),
        ],
        &equal_weights(),
        K,
    );
    let ids: Vec<_> = fused.iter().map(|s| s.id.clone()).collect();
    assert_eq!(ids, vec![id(3), id(5), id(9)]);
}

#[test]
fn a_zero_weight_tier_adds_nothing() {
    let mut weights = equal_weights();
    weights.context = 0.0;
    let fused = fuse(
        vec![
            TierIds::new(RetrievalTier::Context, vec![id(1)]),
            TierIds::new(RetrievalTier::Subject, vec![id(2)]),
        ],
        &weights,
        K,
    );
    assert_eq!(fused[0].id, id(2));
    assert!(fused[1].score.abs() < 1e-12);
}

#[test]
fn an_extension_tier_fuses_like_any_other_and_a_zero_weight_adds_nothing() {
    let tiers = || {
        vec![
            TierIds::new(RetrievalTier::FullText, vec![id(1), id(2)]),
            TierIds::new(RetrievalTier::Extension, vec![id(7), id(2)]),
        ]
    };
    let fused = fuse(tiers(), &equal_weights(), K);
    assert_eq!(fused.len(), 3);
    assert_eq!(fused[0].id, id(2));
    assert!((fused[0].score - (1.0 / 62.0 + 1.0 / 62.0)).abs() < 1e-12);
    assert_eq!(
        fused[1].id,
        id(1),
        "a core rank-1 hit before the extension-only one"
    );
    assert_eq!(fused[2].id, id(7));
    assert!((fused[2].score - 1.0 / 61.0).abs() < 1e-12);
    assert!(
        fused[2]
            .ranks
            .iter()
            .any(|rank| rank.tier == RetrievalTier::Extension)
    );
    assert!(
        !fused[1]
            .ranks
            .iter()
            .any(|rank| rank.tier == RetrievalTier::Extension)
    );

    let zero = fuse(tiers(), &TierWeights::default(), K);
    let core_only = fuse(
        tiers().into_iter().take(1).collect(),
        &TierWeights::default(),
        K,
    );
    assert_eq!(
        zero.len(),
        3,
        "fusion keeps the fact; the recall skips the tier at weight zero"
    );
    assert!(zero[2].score.abs() < f64::EPSILON);
    for (with, without) in zero.iter().zip(&core_only) {
        assert_eq!(with.id, without.id);
        assert!((with.score - without.score).abs() < f64::EPSILON);
    }
}

#[test]
fn attach_keeps_the_fused_order_and_carries_score_and_ranks_onto_the_loaded_fact() {
    let fused = fuse(
        vec![
            TierIds::new(RetrievalTier::Subject, vec![id(1), id(2)]),
            TierIds::new(RetrievalTier::FullText, vec![id(2), id(3)]),
        ],
        &equal_weights(),
        K,
    );
    let expected: Vec<_> = fused.iter().map(|r| (r.id.clone(), r.score)).collect();
    let facts = HashMap::from([
        (id(3), brain_fact(3, "a")),
        (id(1), brain_fact(1, "a")),
        (id(2), brain_fact(2, "a")),
    ]);
    let attached = attach(fused, facts);
    let got: Vec<_> = attached
        .iter()
        .map(|s| (s.fact.id.clone(), s.score))
        .collect();
    assert_eq!(got, expected);
    assert_eq!(attached[0].fact.id, id(2));
    assert_eq!(attached[0].ranks.len(), 2);
}

#[test]
fn attach_drops_a_fused_id_the_load_did_not_return() {
    let fused = fuse(
        vec![TierIds::new(RetrievalTier::Subject, vec![id(1), id(2)])],
        &equal_weights(),
        K,
    );
    let attached = attach(fused, HashMap::from([(id(2), brain_fact(2, "a"))]));
    let got: Vec<_> = attached.iter().map(|s| s.fact.id.clone()).collect();
    assert_eq!(got, vec![id(2)]);
}
