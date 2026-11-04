use std::collections::BTreeMap;

use systemprompt_brain::recall::budget::{BudgetLimits, Candidate, RecallSignal, fill};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{InjectionPlan, RetrievalTier, WithheldReason};

use crate::recall_fixtures::{NAMED, UNNAMED, brain_fact, id, ranked};

const SUBJECT: &[RetrievalTier] = &[RetrievalTier::Subject];
const CONTEXT: &[RetrievalTier] = &[RetrievalTier::Context];
const TEXT: &[RetrievalTier] = &[RetrievalTier::FullText];
const CONTEXT_AND_TEXT: &[RetrievalTier] = &[RetrievalTier::Context, RetrievalTier::FullText];

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

fn requiring_signal() -> BudgetLimits {
    let mut limits = limits(10);
    limits.context_requires_signal = true;
    limits
}

#[test]
fn with_no_lexical_signal_context_only_facts_are_withheld() {
    let plan = fill(
        vec![
            candidate(1, 0.013, 0.9, CONTEXT),
            candidate(2, 0.012, 0.9, CONTEXT),
        ],
        &requiring_signal(),
        UNNAMED,
    );
    assert!(plan.facts.is_empty());
    assert_eq!(
        withheld(&plan, WithheldReason::ContextCap),
        vec![id(1), id(2)]
    );
}

#[test]
fn a_full_text_hit_keeps_the_context_facts() {
    let signal = RecallSignal {
        subject_named: false,
        full_text_hit: true,
    };
    let plan = fill(
        vec![
            candidate(1, 0.013, 0.9, CONTEXT),
            candidate(2, 0.010, 0.9, TEXT),
        ],
        &requiring_signal(),
        signal,
    );
    assert_eq!(ids(&plan), vec![id(1), id(2)]);
}

#[test]
fn a_named_subject_keeps_the_context_facts_under_the_share() {
    let plan = fill(
        vec![
            candidate(1, 0.030, 0.9, SUBJECT),
            candidate(2, 0.013, 0.9, CONTEXT),
        ],
        &requiring_signal(),
        NAMED,
    );
    assert_eq!(ids(&plan), vec![id(1), id(2)]);
}

#[test]
fn a_fact_another_tier_shares_is_never_gated_by_the_signal() {
    let plan = fill(
        vec![
            candidate(1, 0.020, 0.9, CONTEXT_AND_TEXT),
            candidate(2, 0.013, 0.9, CONTEXT),
        ],
        &requiring_signal(),
        UNNAMED,
    );
    assert_eq!(ids(&plan), vec![id(1)]);
    assert_eq!(withheld(&plan, WithheldReason::ContextCap), vec![id(2)]);
}

#[test]
fn the_signal_gate_is_off_by_default() {
    let candidates = vec![
        candidate(1, 0.013, 0.9, CONTEXT),
        candidate(2, 0.012, 0.9, CONTEXT),
    ];
    let plan = fill(candidates.clone(), &limits(10), UNNAMED);
    assert_eq!(ids(&plan), vec![id(1), id(2)]);
    assert!(!limits(10).context_requires_signal);
    assert_eq!(fill(candidates, &limits(10), RecallSignal::default()), plan);
}
