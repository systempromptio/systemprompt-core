//! In-context importance: the per-request score that orders which visible
//! facts fill the token budget, and the terms behind it for the why-view.
//!
//! `total = (base + subject) × workflow × recency`, where `base` is the
//! fact's stored importance, `subject` is `in_context.subject_match` when
//! the subject tier found the fact, `workflow` is the workflow's weight for
//! the fact's kind (1 when none is declared) and `recency` halves every
//! `recency.half_life_days` since the fact last changed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::Serialize;
use systemprompt_identifiers::WorkflowId;
use systemprompt_models::brain::{BrainFact, InContextImportance};

const MINUTES_PER_DAY: f64 = 1_440.0;

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
    pub config: &'a InContextImportance,
    pub workflow: Option<&'a WorkflowId>,
    pub subject_matched: bool,
    pub now: DateTime<Utc>,
}

#[must_use]
pub fn in_context(fact: &BrainFact, input: &ScoreInput<'_>) -> ImportanceTerms {
    let config = input.config;
    let subject = if input.subject_matched {
        config.subject_match
    } else {
        0.0
    };
    let workflow = input
        .workflow
        .and_then(|id| config.workflow_weight.get(id))
        .and_then(|weights| weights.get(&fact.kind))
        .copied()
        .unwrap_or(1.0);
    let recency = recency(fact.updated_at, input.now, config.recency.half_life_days);
    ImportanceTerms {
        base: fact.importance_base,
        subject,
        workflow,
        recency,
        total: (fact.importance_base + subject) * workflow * recency,
    }
}

fn recency(changed: DateTime<Utc>, now: DateTime<Utc>, half_life_days: u32) -> f64 {
    if half_life_days == 0 {
        return 1.0;
    }
    let age_minutes = u32::try_from((now - changed).num_minutes().max(0)).unwrap_or(u32::MAX);
    let age_days = f64::from(age_minutes) / MINUTES_PER_DAY;
    0.5_f64.powf(age_days / f64::from(half_life_days))
}
