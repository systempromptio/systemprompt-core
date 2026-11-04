//! The admission rule the tier SQL applies, restated over a loaded fact.
//!
//! Every core tier admits a fact only when its scope level is one the
//! recall reads, its status is confirmed (or proposed by the principal,
//! or stale when the profile injects stale facts with a marker), it is
//! not due for review as of the recall, it is valid as of the recall and
//! it was observed by then. A fact an extension retriever introduces (see
//! [`super::introduce`]) is loaded by id and never passes that SQL, so
//! [`admitted_by_sql`] applies the identical rule before fusion and fails
//! closed. A rejected fact is withheld under the reason the lifecycle
//! path names (`stale`, `review_due`, `disputed`) and as `scope` where
//! the SQL merely excludes it: a level outside the recall's scopes,
//! another principal's proposed fact, a candidate, retired or superseded
//! fact, one outside its validity window, or one observed after `as_of`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;
use systemprompt_models::brain::{BrainFact, FactStatus, RecallCtx, ScopeLevel, WithheldReason};

/// The principal-side terms of the tier SQL an introduced fact is checked
/// against: the same values `TierFilter` binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admission<'a> {
    pub user: &'a UserId,
    pub scopes: &'a [ScopeLevel],
    pub include_stale: bool,
    pub as_of: DateTime<Utc>,
}

impl<'a> Admission<'a> {
    #[must_use]
    pub fn of(ctx: &'a RecallCtx, include_stale: bool) -> Self {
        Self {
            user: &ctx.user_id,
            scopes: &ctx.scopes,
            include_stale,
            as_of: ctx.as_of,
        }
    }
}

#[must_use]
pub fn admitted_by_sql(fact: &BrainFact, admission: &Admission<'_>) -> Option<WithheldReason> {
    if !admission.scopes.contains(&fact.scope.level()) {
        return Some(WithheldReason::Scope);
    }
    if let Some(reason) = status_withheld(fact, admission) {
        return Some(reason);
    }
    if fact.is_review_due(admission.as_of) {
        return Some(WithheldReason::ReviewDue);
    }
    if !fact.is_valid_at(admission.as_of) {
        return Some(WithheldReason::Scope);
    }
    if fact.observed_at > admission.as_of {
        return Some(WithheldReason::Scope);
    }
    None
}

fn status_withheld(fact: &BrainFact, admission: &Admission<'_>) -> Option<WithheldReason> {
    match fact.status {
        FactStatus::Confirmed => None,
        FactStatus::Proposed if fact.owner_user_id.as_ref() == Some(admission.user) => None,
        FactStatus::Proposed => Some(WithheldReason::Scope),
        FactStatus::Stale if admission.include_stale => None,
        FactStatus::Stale => Some(WithheldReason::Stale),
        FactStatus::Disputed => Some(WithheldReason::Disputed),
        FactStatus::Candidate | FactStatus::Superseded | FactStatus::Retired => {
            Some(WithheldReason::Scope)
        },
    }
}
