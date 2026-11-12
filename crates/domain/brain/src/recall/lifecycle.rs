//! The lifecycle half of recall's verdict: whether a fact's status,
//! review date or validity window withholds it at an instant, independent
//! of who asks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::brain::{BrainFact, FactStatus, WithheldReason};

#[must_use]
pub fn lifecycle_withheld(
    fact: &BrainFact,
    now: chrono::DateTime<chrono::Utc>,
    inject_stale: bool,
) -> Option<WithheldReason> {
    if fact.status == FactStatus::Stale && !inject_stale {
        return Some(WithheldReason::Stale);
    }
    if fact.is_review_due(now) {
        return Some(WithheldReason::ReviewDue);
    }
    if fact.status == FactStatus::Disputed {
        return Some(WithheldReason::Disputed);
    }
    if !fact.is_valid_at(now)
        || matches!(
            fact.status,
            FactStatus::Candidate | FactStatus::Retired | FactStatus::Superseded
        )
    {
        return Some(WithheldReason::Scope);
    }
    None
}
