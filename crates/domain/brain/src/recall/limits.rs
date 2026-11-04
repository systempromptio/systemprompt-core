//! The row limits one recall hands its tiers.
//!
//! From the retrieval declaration: the shared tier limit (`max_facts × 2`,
//! clamped to 8..=50), the pinned tier's (`max_facts`, at most 50) and the
//! subject and linked tiers' own (`subject_limit` when declared, at most
//! `MAX_SUBJECT_LIMIT`; otherwise the shared one). Every id a tier
//! returns is loaded once, so the most facts one recall loads is the sum
//! of the five limits plus the extension candidates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::brain::{MAX_SUBJECT_LIMIT, RetrievalDoc};

const MIN_TIER_LIMIT: u32 = 8;
const MAX_TIER_LIMIT: u32 = 50;

/// The limits of one recall's tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierLimits {
    pub tier: u32,
    pub pinned: u32,
    pub subject: u32,
}

#[must_use]
pub fn tier_limits(retrieval: &RetrievalDoc) -> TierLimits {
    let tier = retrieval
        .max_facts
        .saturating_mul(2)
        .clamp(MIN_TIER_LIMIT, MAX_TIER_LIMIT);
    let subject = match retrieval.subject_limit {
        0 => tier,
        declared => declared.min(MAX_SUBJECT_LIMIT),
    };
    TierLimits {
        tier,
        pinned: retrieval.max_facts.min(MAX_TIER_LIMIT),
        subject,
    }
}
