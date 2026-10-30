//! [`FormulaImportance`]: core's importance formula, used when no
//! extension model scores a candidate.
//!
//! The stored base is `kind_weight × source_weight + hint × hint_weight +
//! confirmed_bonus`, where a missing kind or source weight counts as `1.0`
//! and the hint is the model-scored field clamped to `0.0..=1.0`. The
//! per-request score is `base × workflow_weight × recency + subject_match +
//! usage`, with `recency = 0.5^(age_days / half_life_days)` and `usage =
//! cited_rate × cited_rate_weight − ignored_rate × ignored_penalty`.
//!
//! The seam's `in_context` sees no tier ranks or usage history, so it
//! scores with no subject match and zero usage; recall itself scores
//! through `recall::score`, which knows whether the subject tier matched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::brain::ImportanceModel;
use systemprompt_identifiers::{FactKindId, WorkflowId};
use systemprompt_models::brain::{BrainFact, FactCandidate, ImportanceConfig, RecallCtx};

/// Core's importance formula.
#[derive(Debug, Clone, Copy, Default)]
pub struct FormulaImportance;

/// The per-request inputs of a fact's score.
#[derive(Debug, Clone, PartialEq)]
pub struct InContextTerms<'a> {
    pub base: f64,
    pub kind: &'a FactKindId,
    pub workflow: Option<&'a WorkflowId>,
    pub age_days: f64,
    pub subject_matched: bool,
    pub cited_rate: f64,
    pub ignored_rate: f64,
}

impl FormulaImportance {
    pub const NAME: &'static str = "core.formula";

    #[must_use]
    pub fn base_with(candidate: &FactCandidate, config: &ImportanceConfig, confirmed: bool) -> f64 {
        let base = &config.base;
        let kind = base
            .kind_weight
            .get(&candidate.kind)
            .copied()
            .unwrap_or(1.0);
        let source = base
            .source_weight
            .get(&candidate.source.class())
            .copied()
            .unwrap_or(1.0);
        let hint = base.model_hint.as_ref().map_or(0.0, |hint| {
            let value = candidate
                .fields
                .get(&hint.field)
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);
            value.clamp(0.0, 1.0) * hint.weight
        });
        let bonus = if confirmed {
            base.confirmed_by_owner
        } else {
            0.0
        };
        kind.mul_add(source, hint) + bonus
    }

    #[must_use]
    pub fn score(terms: &InContextTerms<'_>, config: &ImportanceConfig) -> f64 {
        let ctx = &config.in_context;
        let workflow = terms
            .workflow
            .and_then(|id| ctx.workflow_weight.get(id))
            .and_then(|kinds| kinds.get(terms.kind))
            .copied()
            .unwrap_or(1.0);
        let half_life = f64::from(ctx.recency.half_life_days.max(1));
        let recency = 0.5_f64.powf(terms.age_days.max(0.0) / half_life);
        let subject = if terms.subject_matched {
            ctx.subject_match
        } else {
            0.0
        };
        let usage = terms.cited_rate * ctx.usage.cited_rate_weight
            - terms.ignored_rate * ctx.usage.ignored_penalty;
        terms.base * workflow * recency + subject + usage
    }
}

impl ImportanceModel for FormulaImportance {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn base(&self, candidate: &FactCandidate, config: &ImportanceConfig) -> Option<f64> {
        Some(Self::base_with(candidate, config, false))
    }

    fn in_context(
        &self,
        fact: &BrainFact,
        ctx: &RecallCtx,
        config: &ImportanceConfig,
    ) -> Option<f64> {
        let terms = InContextTerms {
            base: fact.importance_base,
            kind: &fact.kind,
            workflow: ctx.workflow.as_ref(),
            age_days: age_days(fact, ctx),
            subject_matched: false,
            cited_rate: 0.0,
            ignored_rate: 0.0,
        };
        Some(Self::score(&terms, config))
    }
}

const MINUTES_PER_DAY: f64 = 1_440.0;

#[must_use]
pub fn age_days(fact: &BrainFact, ctx: &RecallCtx) -> f64 {
    let minutes =
        u32::try_from((ctx.now - fact.updated_at).num_minutes().max(0)).unwrap_or(u32::MAX);
    f64::from(minutes) / MINUTES_PER_DAY
}
