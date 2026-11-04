use std::collections::BTreeMap;

use systemprompt_brain::jobs::{UsageRates, laplace_rates};
use systemprompt_brain::recall::budget::{BudgetLimits, Candidate, fill, usage_term};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{
    FillOrder, InjectionPlan, RetrievalDoc, RetrievalTier, WithheldReason,
};

use crate::recall_fixtures::{brain_fact, id, ranked};

const TEXT: &[RetrievalTier] = &[RetrievalTier::FullText];
const ZERO: u64 = 0.0f64.to_bits();

fn candidate(n: u32, score: f64, importance: f64, rates: UsageRates) -> Candidate {
    let mut fact = brain_fact(n, "a");
    fact.usage_cited_rate = rates.cited_rate;
    fact.usage_ignored_rate = rates.ignored_rate;
    Candidate {
        scored: ranked(fact, score, TEXT),
        text: "xxxx".to_owned(),
        importance,
    }
}

fn no_rates() -> UsageRates {
    UsageRates {
        cited_rate: 0.0,
        ignored_rate: 0.0,
    }
}

fn limits(usage_weight: f64) -> BudgetLimits {
    let mut limits = BudgetLimits::plain(BTreeMap::new(), 10, 1000);
    limits.usage_weight = usage_weight;
    limits
}

fn ids(plan: &InjectionPlan) -> Vec<BrainFactId> {
    plan.facts.iter().map(|s| s.fact.id.clone()).collect()
}

fn scores(plan: &InjectionPlan) -> Vec<f64> {
    plan.facts.iter().map(|s| s.score).collect()
}

fn siblings() -> Vec<Candidate> {
    vec![
        candidate(1, 0.016, 0.50, laplace_rates(8, 0)),
        candidate(2, 0.016, 0.50, laplace_rates(8, 8)),
        candidate(3, 0.016, 0.50, UsageRates::prior()),
    ]
}

#[test]
fn the_knobs_default_to_off() {
    let doc = RetrievalDoc::default();
    assert!(doc.usage_weight.abs() < f64::EPSILON);
    assert_eq!(doc.usage_min_recalls, 0);
    assert!(limits(0.0).usage_weight.abs() < f64::EPSILON);
}

#[test]
fn a_fact_with_no_history_adds_exactly_zero_at_any_weight() {
    let prior = candidate(1, 0.016, 0.5, UsageRates::prior());
    let unrated = candidate(2, 0.016, 0.5, no_rates());
    for weight in [0.0, 0.01, 0.5, 1.0] {
        assert_eq!(usage_term(&prior.scored.fact, weight).to_bits(), ZERO);
        assert_eq!(usage_term(&unrated.scored.fact, weight).to_bits(), ZERO);
    }
    let plan = fill(vec![prior, unrated], &limits(1.0), false);
    assert_eq!(scores(&plan), vec![0.016, 0.016]);
}

#[test]
fn a_cited_fact_outranks_an_ignored_sibling_when_the_weight_is_on() {
    let plan = fill(siblings(), &limits(0.01), false);
    assert_eq!(ids(&plan), vec![id(2), id(3), id(1)]);
    let &[cited, prior, ignored] = scores(&plan).as_slice() else {
        panic!("three facts");
    };
    assert!(cited > 0.016);
    assert_eq!(prior.to_bits(), 0.016f64.to_bits());
    assert!(ignored < 0.016);
}

#[test]
fn a_zero_weight_leaves_the_p2_plan_and_its_scores_untouched() {
    // Why: IEEE 754 gives 0.0 × −0.8 = −0.0, which total_cmp orders below +0.0;
    // the term normalises so a zero weight is bit-identical to no term at all.
    let ignored = siblings().remove(0);
    assert_eq!(usage_term(&ignored.scored.fact, 0.0).to_bits(), ZERO);
    let expected = fill(
        siblings(),
        &BudgetLimits::plain(BTreeMap::new(), 10, 1000),
        false,
    );
    assert_eq!(ids(&expected), vec![id(1), id(2), id(3)]);
    assert_eq!(fill(siblings(), &limits(0.0), false), expected);
    assert_eq!(scores(&expected), vec![0.016; 3]);
}

#[test]
fn the_usage_term_breaks_a_tie_under_importance_order_too() {
    let mut limits = limits(0.01);
    limits.order = FillOrder::Importance;
    let plan = fill(siblings(), &limits, false);
    assert_eq!(ids(&plan), vec![id(2), id(3), id(1)]);
}

#[test]
fn the_usage_term_is_seen_by_the_low_score_gate() {
    let mut limits = limits(0.01);
    limits.min_fused_score = 0.016;
    let plan = fill(siblings(), &limits, false);
    assert_eq!(ids(&plan), vec![id(2), id(3)]);
    let low: Vec<BrainFactId> = plan
        .withheld
        .iter()
        .filter(|w| w.reason == WithheldReason::LowScore)
        .map(|w| w.id.clone())
        .collect();
    assert_eq!(low, vec![id(1)]);
}

#[test]
fn the_fill_is_deterministic_over_input_order_with_usage_on() {
    let mut reversed = siblings();
    reversed.reverse();
    assert_eq!(
        fill(siblings(), &limits(0.01), false),
        fill(reversed, &limits(0.01), false)
    );
}
