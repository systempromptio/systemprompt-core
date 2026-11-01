use std::collections::BTreeMap;

use systemprompt_brain::recall::budget::{BudgetLimits, Candidate, estimate_tokens, fill};
use systemprompt_identifiers::WorkflowId;
use systemprompt_models::brain::{RetrievalDoc, WithheldReason, WorkflowProfileDecl};

use crate::fixtures::kind;
use crate::recall_fixtures::{brain_fact, id, scored};

fn candidate(n: u32, kind_id: &str, importance: f64, text_chars: usize) -> Candidate {
    Candidate {
        scored: scored(brain_fact(n, kind_id), 0.01),
        text: "x".repeat(text_chars),
        importance,
    }
}

fn limits(quotas: &[(&str, u32)], max_facts: u32, budget_tokens: u32) -> BudgetLimits {
    BudgetLimits {
        quotas: quotas.iter().map(|(k, q)| (kind(k), *q)).collect(),
        max_facts,
        budget_tokens,
    }
}

fn reasons(plan: &systemprompt_models::brain::InjectionPlan) -> Vec<WithheldReason> {
    plan.withheld.iter().map(|w| w.reason).collect()
}

#[test]
fn tokens_are_a_quarter_of_the_characters_rounded_up() {
    assert_eq!(estimate_tokens(""), 0);
    assert_eq!(estimate_tokens("abcd"), 1);
    assert_eq!(estimate_tokens("abcde"), 2);
    assert_eq!(estimate_tokens("ééééé"), 2);
}

#[test]
fn facts_fill_in_descending_importance() {
    let plan = fill(
        vec![
            candidate(1, "a", 0.1, 4),
            candidate(2, "a", 0.9, 4),
            candidate(3, "a", 0.5, 4),
        ],
        &limits(&[], 10, 100),
    );
    let ids: Vec<_> = plan.facts.iter().map(|s| s.fact.id.clone()).collect();
    assert_eq!(ids, vec![id(2), id(3), id(1)]);
    assert_eq!(plan.tokens, 3);
}

#[test]
fn a_kind_over_its_quota_is_withheld_as_quota() {
    let plan = fill(
        vec![
            candidate(1, "a", 0.9, 4),
            candidate(2, "a", 0.8, 4),
            candidate(3, "b", 0.7, 4),
            candidate(4, "a", 0.6, 4),
        ],
        &limits(&[("a", 1)], 10, 100),
    );
    let ids: Vec<_> = plan.facts.iter().map(|s| s.fact.id.clone()).collect();
    assert_eq!(ids, vec![id(1), id(3)]);
    assert_eq!(
        reasons(&plan),
        vec![WithheldReason::Quota, WithheldReason::Quota]
    );
}

#[test]
fn the_token_cap_withholds_what_does_not_fit_and_keeps_smaller_facts() {
    let plan = fill(
        vec![
            candidate(1, "a", 0.9, 40),
            candidate(2, "a", 0.8, 40),
            candidate(3, "a", 0.7, 4),
        ],
        &limits(&[], 10, 12),
    );
    let ids: Vec<_> = plan.facts.iter().map(|s| s.fact.id.clone()).collect();
    assert_eq!(ids, vec![id(1), id(3)]);
    assert_eq!(plan.tokens, 11);
    assert_eq!(reasons(&plan), vec![WithheldReason::Budget]);
}

#[test]
fn max_facts_caps_the_count() {
    let plan = fill(
        vec![candidate(1, "a", 0.9, 4), candidate(2, "a", 0.8, 4)],
        &limits(&[], 1, 100),
    );
    assert_eq!(plan.facts.len(), 1);
    assert_eq!(reasons(&plan), vec![WithheldReason::Budget]);
}

#[test]
fn a_workflow_overrides_quotas_and_caps_the_budget() {
    let retrieval = RetrievalDoc {
        quotas: BTreeMap::from([(kind("a"), 5), (kind("b"), 5)]),
        ..RetrievalDoc::default()
    };
    let workflow = WorkflowProfileDecl {
        id: WorkflowId::try_new("delivery").expect("workflow"),
        name: "Delivery".to_owned(),
        kinds: vec![kind("a")],
        confirm_role: None,
        quotas: BTreeMap::from([(kind("a"), 1)]),
        budget_tokens: Some(200),
    };
    let resolved = BudgetLimits::resolve(&retrieval, Some(&workflow), 800);
    assert_eq!(resolved.quotas.get(&kind("a")), Some(&1));
    assert_eq!(resolved.quotas.get(&kind("b")), Some(&5));
    assert_eq!(resolved.budget_tokens, 200);
    assert_eq!(
        BudgetLimits::resolve(&retrieval, None, 800).budget_tokens,
        800
    );
}
