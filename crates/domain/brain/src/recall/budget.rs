//! Core injection fill: the visible facts in descending in-context
//! importance (ties by fused score, then fact id) are taken while their
//! kind is under its quota, the fact count is under `max_facts` and the
//! estimated tokens fit the budget. A fact over its kind's quota is
//! withheld as `Quota`; one that does not fit as `Budget`. Tokens are
//! estimated as a quarter of the line's characters, rounded up.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::FactKindId;
use systemprompt_models::brain::{
    InjectionPlan, RetrievalDoc, ScoredFact, WithheldFact, WithheldReason, WorkflowProfileDecl,
};

/// The limits one recall fills against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetLimits {
    pub quotas: BTreeMap<FactKindId, u32>,
    pub max_facts: u32,
    pub budget_tokens: u32,
}

impl BudgetLimits {
    #[must_use]
    pub fn resolve(
        retrieval: &RetrievalDoc,
        workflow: Option<&WorkflowProfileDecl>,
        budget_tokens: u32,
    ) -> Self {
        let mut quotas = retrieval.quotas.clone();
        let mut budget = budget_tokens;
        if let Some(workflow) = workflow {
            quotas.extend(workflow.quotas.iter().map(|(k, v)| (k.clone(), *v)));
            if let Some(cap) = workflow.budget_tokens {
                budget = budget.min(cap);
            }
        }
        Self {
            quotas,
            max_facts: retrieval.max_facts,
            budget_tokens: budget,
        }
    }
}

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

#[must_use]
pub fn fill(mut candidates: Vec<Candidate>, limits: &BudgetLimits) -> InjectionPlan {
    candidates.sort_by(|a, b| {
        b.importance
            .total_cmp(&a.importance)
            .then_with(|| b.scored.score.total_cmp(&a.scored.score))
            .then_with(|| a.scored.fact.id.cmp(&b.scored.fact.id))
    });
    let mut plan = InjectionPlan::default();
    let mut per_kind: BTreeMap<FactKindId, u32> = BTreeMap::new();
    for candidate in candidates {
        let fact = &candidate.scored.fact;
        let taken = per_kind.get(&fact.kind).copied().unwrap_or(0);
        let reason = if limits.quotas.get(&fact.kind).is_some_and(|q| taken >= *q) {
            Some(WithheldReason::Quota)
        } else {
            let tokens = estimate_tokens(&candidate.text);
            let full = u32::try_from(plan.facts.len()).unwrap_or(u32::MAX) >= limits.max_facts;
            if full || plan.tokens.saturating_add(tokens) > limits.budget_tokens {
                Some(WithheldReason::Budget)
            } else {
                plan.tokens += tokens;
                None
            }
        };
        match reason {
            Some(reason) => plan.withheld.push(WithheldFact {
                id: fact.id.clone(),
                reason,
            }),
            None => {
                *per_kind.entry(fact.kind.clone()).or_default() += 1;
                plan.facts.push(candidate.scored);
            },
        }
    }
    plan
}
