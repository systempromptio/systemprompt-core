use systemprompt_brain::recall::limits::{TierLimits, tier_limits};
use systemprompt_models::brain::{MAX_SUBJECT_LIMIT, RetrievalDoc};

fn retrieval(max_facts: u32, subject_limit: u32) -> RetrievalDoc {
    RetrievalDoc {
        max_facts,
        subject_limit,
        ..RetrievalDoc::default()
    }
}

#[test]
fn the_undeclared_subject_limit_is_the_shared_tier_limit() {
    assert_eq!(
        tier_limits(&retrieval(12, 0)),
        TierLimits {
            tier: 24,
            pinned: 12,
            subject: 24,
        }
    );
    assert_eq!(tier_limits(&retrieval(2, 0)).tier, 8, "clamped up");
    let wide = tier_limits(&retrieval(60, 0));
    assert_eq!((wide.tier, wide.pinned, wide.subject), (50, 50, 50));
}

#[test]
fn a_declared_subject_limit_moves_only_the_subject_tiers() {
    let limits = tier_limits(&retrieval(12, 50));
    assert_eq!((limits.tier, limits.pinned, limits.subject), (24, 12, 50));
    assert_eq!(
        tier_limits(&retrieval(12, 500)).subject,
        MAX_SUBJECT_LIMIT,
        "never past the maximum"
    );
}
