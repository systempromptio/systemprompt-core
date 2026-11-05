use std::collections::BTreeMap;

use systemprompt_brain::recall::budget::{BudgetLimits, Candidate, fill};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{FillOrder, InjectionPlan, RetrievalTier, WithheldReason};

use crate::recall_fixtures::{NAMED, UNNAMED, brain_fact, id, ranked};

const SUBJECT: &[RetrievalTier] = &[RetrievalTier::Subject];
const LINKED: &[RetrievalTier] = &[RetrievalTier::Linked];
const BOTH: &[RetrievalTier] = &[RetrievalTier::Subject, RetrievalTier::Linked];
const CONTEXT: &[RetrievalTier] = &[RetrievalTier::Context];
const TEXT: &[RetrievalTier] = &[RetrievalTier::FullText];

fn candidate(n: u32, score: f64, importance: f64, tiers: &[RetrievalTier]) -> Candidate {
    Candidate {
        scored: ranked(brain_fact(n, "a"), score, tiers),
        text: "xxxx".to_owned(),
        importance,
    }
}

fn limits(max_facts: u32) -> BudgetLimits {
    BudgetLimits::plain(BTreeMap::new(), max_facts, 1000)
}

fn ids(plan: &InjectionPlan) -> Vec<BrainFactId> {
    plan.facts.iter().map(|s| s.fact.id.clone()).collect()
}

fn withheld(plan: &InjectionPlan, reason: WithheldReason) -> Vec<BrainFactId> {
    plan.withheld
        .iter()
        .filter(|w| w.reason == reason)
        .map(|w| w.id.clone())
        .collect()
}

fn cross_entity() -> Vec<Candidate> {
    vec![
        candidate(1, 0.030, 0.30, BOTH),
        candidate(2, 0.026, 0.35, LINKED),
        candidate(3, 0.016, 0.95, TEXT),
        candidate(4, 0.016, 0.90, TEXT),
        candidate(5, 0.010, 0.85, CONTEXT),
    ]
}

#[test]
fn rank_order_keeps_the_facts_the_tiers_agreed_on_when_the_count_cuts() {
    let plan = fill(cross_entity(), &limits(2), NAMED);
    assert_eq!(ids(&plan), vec![id(1), id(2)]);
    assert_eq!(
        withheld(&plan, WithheldReason::MaxFacts),
        vec![id(3), id(4), id(5)]
    );
}

#[test]
fn importance_order_is_the_pre_p2_fill_and_cuts_them() {
    let mut limits = limits(2);
    limits.order = FillOrder::Importance;
    let plan = fill(cross_entity(), &limits, NAMED);
    assert_eq!(ids(&plan), vec![id(3), id(4)]);
}

#[test]
fn a_rank_tie_falls_back_to_importance_then_id() {
    let plan = fill(
        vec![
            candidate(9, 0.016, 0.50, TEXT),
            candidate(8, 0.016, 0.50, TEXT),
            candidate(7, 0.016, 0.70, TEXT),
        ],
        &limits(10),
        UNNAMED,
    );
    assert_eq!(ids(&plan), vec![id(7), id(8), id(9)]);
}

#[test]
fn the_linked_share_holds_slots_for_link_hits_when_a_subject_is_named() {
    let mut limits = limits(3);
    limits.linked_share = 0.34;
    let candidates = vec![
        candidate(1, 0.030, 0.5, SUBJECT),
        candidate(2, 0.028, 0.5, TEXT),
        candidate(3, 0.027, 0.5, TEXT),
        candidate(4, 0.020, 0.5, LINKED),
    ];
    let plan = fill(candidates.clone(), &limits, NAMED);
    assert_eq!(ids(&plan), vec![id(1), id(2), id(4)]);
    assert_eq!(withheld(&plan, WithheldReason::MaxFacts), vec![id(3)]);
    let unnamed = fill(candidates, &limits, UNNAMED);
    assert_eq!(unnamed.facts.len(), 3);
    assert_eq!(ids(&unnamed), vec![id(1), id(2), id(3)]);
}

#[test]
fn the_reserve_never_exceeds_the_link_hits_present() {
    let mut limits = limits(3);
    limits.linked_share = 1.0;
    let plan = fill(
        vec![
            candidate(1, 0.030, 0.5, SUBJECT),
            candidate(2, 0.028, 0.5, TEXT),
            candidate(3, 0.020, 0.5, LINKED),
        ],
        &limits,
        NAMED,
    );
    assert_eq!(ids(&plan), vec![id(1), id(2), id(3)]);
}

#[test]
fn a_linked_fact_withheld_for_its_quota_releases_its_slot() {
    let mut limits = BudgetLimits::plain(
        [(crate::fixtures::kind("a"), 1)].into_iter().collect(),
        2,
        1000,
    );
    limits.linked_share = 0.5;
    let plan = fill(
        vec![
            candidate(1, 0.030, 0.5, LINKED),
            candidate(2, 0.028, 0.5, TEXT),
            candidate(3, 0.020, 0.5, LINKED),
        ],
        &limits,
        NAMED,
    );
    assert_eq!(withheld(&plan, WithheldReason::Quota), vec![id(2), id(3)]);
    assert_eq!(ids(&plan), vec![id(1)]);
}

#[test]
fn facts_under_the_minimum_fused_score_are_withheld_as_low_score() {
    let mut limits = limits(10);
    limits.min_fused_score = 0.02;
    let plan = fill(
        vec![
            candidate(1, 0.030, 0.9, SUBJECT),
            candidate(2, 0.016, 0.9, TEXT),
            candidate(3, 0.010, 0.9, CONTEXT),
        ],
        &limits,
        NAMED,
    );
    assert_eq!(ids(&plan), vec![id(1)]);
    assert_eq!(
        withheld(&plan, WithheldReason::LowScore),
        vec![id(2), id(3)]
    );
}

#[test]
fn a_message_no_tier_scores_injects_nothing() {
    let mut limits = limits(10);
    limits.min_fused_score = 0.02;
    let plan = fill(
        vec![
            candidate(1, 0.016, 0.9, TEXT),
            candidate(2, 0.010, 0.9, CONTEXT),
        ],
        &limits,
        UNNAMED,
    );
    assert!(plan.facts.is_empty());
    assert_eq!(plan.tokens, 0);
    assert_eq!(withheld(&plan, WithheldReason::LowScore).len(), 2);
}

#[test]
fn a_zero_minimum_withholds_nothing() {
    let plan = fill(vec![candidate(1, 0.0, 0.9, CONTEXT)], &limits(10), UNNAMED);
    assert_eq!(ids(&plan), vec![id(1)]);
}

#[test]
fn context_only_facts_are_capped_when_a_subject_is_named() {
    let mut limits = limits(6);
    limits.context_share = 0.34;
    let candidates = vec![
        candidate(1, 0.030, 0.5, SUBJECT),
        candidate(2, 0.013, 0.5, CONTEXT),
        candidate(3, 0.012, 0.5, CONTEXT),
        candidate(4, 0.011, 0.5, CONTEXT),
        candidate(
            5,
            0.010,
            0.5,
            &[RetrievalTier::Context, RetrievalTier::FullText],
        ),
    ];
    let plan = fill(candidates.clone(), &limits, NAMED);
    assert_eq!(ids(&plan), vec![id(1), id(2), id(3), id(5)]);
    assert_eq!(withheld(&plan, WithheldReason::ContextCap), vec![id(4)]);
    let unnamed = fill(candidates, &limits, UNNAMED);
    assert_eq!(unnamed.facts.len(), 5);
}

#[test]
fn a_zero_context_share_admits_context_facts_only_without_a_subject() {
    let mut limits = limits(6);
    limits.context_share = 0.0;
    let plan = fill(
        vec![
            candidate(1, 0.030, 0.5, SUBJECT),
            candidate(2, 0.013, 0.5, CONTEXT),
        ],
        &limits,
        NAMED,
    );
    assert_eq!(ids(&plan), vec![id(1)]);
    assert_eq!(withheld(&plan, WithheldReason::ContextCap), vec![id(2)]);
}

#[test]
fn the_fill_is_deterministic_over_input_order() {
    let mut limits = limits(3);
    limits.linked_share = 0.34;
    limits.context_share = 0.34;
    limits.min_fused_score = 0.005;
    let forward = fill(cross_entity(), &limits, NAMED);
    let mut reversed = cross_entity();
    reversed.reverse();
    assert_eq!(fill(reversed, &limits, NAMED), forward);
}
