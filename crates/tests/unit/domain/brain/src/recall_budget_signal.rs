use std::collections::BTreeMap;

use systemprompt_brain::recall::budget::{BudgetLimits, Candidate, RecallSignal, fill};
use systemprompt_brain::recall::signal::signal_of;
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
        kind_asked: false,
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
fn a_full_text_hit_alone_does_not_open_the_context_cap_when_declared_so() {
    let mut limits = requiring_signal();
    limits.full_text_opens_context = false;
    let hit_only = RecallSignal {
        full_text_hit: true,
        ..RecallSignal::default()
    };
    let plan = fill(
        vec![
            candidate(1, 0.02, 0.9, TEXT),
            candidate(2, 0.013, 0.9, CONTEXT),
            candidate(3, 0.012, 0.9, CONTEXT_AND_TEXT),
        ],
        &limits,
        hit_only,
    );
    assert_eq!(
        ids(&plan),
        vec![id(1), id(3)],
        "the hits inject, the context-only fact does not"
    );
    assert_eq!(withheld(&plan, WithheldReason::ContextCap), vec![id(2)]);
    let asked = RecallSignal {
        full_text_hit: true,
        kind_asked: true,
        ..RecallSignal::default()
    };
    let plan = fill(vec![candidate(2, 0.013, 0.9, CONTEXT)], &limits, asked);
    assert_eq!(ids(&plan), vec![id(2)], "an asked kind still opens it");
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

#[test]
fn an_asked_kind_is_a_signal_that_keeps_the_context_facts() {
    let signal = RecallSignal {
        kind_asked: true,
        ..RecallSignal::default()
    };
    let plan = fill(
        vec![candidate(1, 0.013, 0.9, CONTEXT)],
        &requiring_signal(),
        signal,
    );
    assert_eq!(ids(&plan), vec![id(1)]);
    assert!(signal.any() && !RecallSignal::default().any());
}

const CONTEXT_AND_EXTENSION: &[RetrievalTier] = &[RetrievalTier::Context, RetrievalTier::Extension];
const EXTENSION: &[RetrievalTier] = &[RetrievalTier::Extension];

#[test]
fn the_extension_tier_is_never_a_signal() {
    let fused = vec![
        ranked(brain_fact(1, "a"), 0.02, EXTENSION),
        ranked(brain_fact(2, "a"), 0.01, CONTEXT_AND_EXTENSION),
    ];
    assert_eq!(signal_of(&fused, false), RecallSignal::default());
    assert!(signal_of(&[ranked(brain_fact(3, "a"), 0.01, TEXT)], false).full_text_hit);
}

#[test]
fn an_extension_rank_does_not_make_a_context_fact_shared() {
    let plan = fill(
        vec![candidate(1, 0.02, 0.9, CONTEXT_AND_EXTENSION)],
        &requiring_signal(),
        UNNAMED,
    );
    assert_eq!(withheld(&plan, WithheldReason::ContextCap), vec![id(1)]);
    let open = fill(
        vec![candidate(1, 0.02, 0.9, CONTEXT_AND_EXTENSION)],
        &limits(10),
        UNNAMED,
    );
    assert_eq!(ids(&open), vec![id(1)], "gate off, as before");
}

#[test]
fn with_no_signal_an_introduced_fact_is_withheld_as_no_signal() {
    let candidates = vec![
        candidate(1, 0.02, 0.9, EXTENSION),
        candidate(2, 0.03, 0.9, CONTEXT_AND_TEXT),
    ];
    let plan = fill(candidates.clone(), &requiring_signal(), UNNAMED);
    assert_eq!(withheld(&plan, WithheldReason::NoSignal), vec![id(1)]);
    assert_eq!(ids(&plan), vec![id(2)], "a core rank keeps it");
    let signalled = fill(
        candidates.clone(),
        &requiring_signal(),
        RecallSignal {
            full_text_hit: true,
            ..RecallSignal::default()
        },
    );
    assert_eq!(ids(&signalled), vec![id(2), id(1)]);
    assert_eq!(fill(candidates, &limits(10), UNNAMED).facts.len(), 2);
}
