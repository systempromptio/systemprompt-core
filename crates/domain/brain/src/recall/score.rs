//! In-context importance as recall orders facts by it, and the terms behind
//! it for the why-view. The formula is [`FormulaImportance::score`]; this
//! module supplies what recall knows per request (whether the subject tier
//! found the fact) and breaks the score into its terms. Usage rates are not
//! read on the recall path, so they contribute zero.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_models::brain::{BrainFact, ImportanceConfig, RecallCtx};

use crate::defaults::{FormulaImportance, InContextTerms, age_days};

/// The terms of one fact's in-context importance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ImportanceTerms {
    pub base: f64,
    pub subject: f64,
    pub workflow: f64,
    pub recency: f64,
    pub total: f64,
}

/// What the score is computed against.
#[derive(Debug, Clone, Copy)]
pub struct ScoreInput<'a> {
    pub config: &'a ImportanceConfig,
    pub ctx: &'a RecallCtx,
    pub subject_matched: bool,
}

#[must_use]
pub fn in_context(fact: &BrainFact, input: &ScoreInput<'_>) -> ImportanceTerms {
    let terms = InContextTerms {
        base: fact.importance_base,
        kind: &fact.kind,
        workflow: input.ctx.workflow.as_ref(),
        age_days: age_days(fact, input.ctx),
        subject_matched: input.subject_matched,
        cited_rate: 0.0,
        ignored_rate: 0.0,
    };
    let total = FormulaImportance::score(&terms, input.config);
    let unit = |terms: InContextTerms<'_>| FormulaImportance::score(&terms, input.config);
    let subject = unit(InContextTerms {
        base: 0.0,
        ..terms.clone()
    });
    let workflow = unit(InContextTerms {
        base: 1.0,
        age_days: 0.0,
        subject_matched: false,
        ..terms.clone()
    });
    let recency = unit(InContextTerms {
        base: 1.0,
        workflow: None,
        subject_matched: false,
        ..terms.clone()
    });
    ImportanceTerms {
        base: fact.importance_base,
        subject,
        workflow,
        recency,
        total,
    }
}
