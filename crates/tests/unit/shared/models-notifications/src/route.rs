use systemprompt_identifiers::{NotificationStreamId, UserId};
use systemprompt_models::notifications::{
    Attention, AttentionReason, RecipientDecision, RoutePlan,
};

fn decision(user: &str, attention: Attention, reason: AttentionReason) -> RecipientDecision {
    RecipientDecision {
        user: UserId::new(user),
        attention,
        reason,
    }
}

#[test]
fn merging_keeps_the_loudest_decision_per_user() {
    let mut plan = RoutePlan {
        recipients: vec![
            decision("a", Attention::Agent, AttentionReason::Default),
            decision("b", Attention::Human, AttentionReason::Sender),
        ],
        stream: None,
    };
    plan.merge(RoutePlan {
        recipients: vec![
            decision("a", Attention::Human, AttentionReason::Sender),
            decision("b", Attention::Silent, AttentionReason::Default),
            decision("c", Attention::Silent, AttentionReason::Default),
        ],
        stream: None,
    });
    assert_eq!(
        plan.recipients,
        vec![
            decision("a", Attention::Human, AttentionReason::Sender),
            decision("b", Attention::Human, AttentionReason::Sender),
            decision("c", Attention::Silent, AttentionReason::Default),
        ]
    );
}

#[test]
fn an_equal_attention_keeps_the_first_reason() {
    let mut plan = RoutePlan {
        recipients: vec![decision("a", Attention::Agent, AttentionReason::Sender)],
        stream: None,
    };
    plan.merge(RoutePlan {
        recipients: vec![decision("a", Attention::Agent, AttentionReason::Default)],
        stream: None,
    });
    assert_eq!(plan.recipients[0].reason, AttentionReason::Sender);
}

#[test]
fn merging_adopts_a_stream_only_when_none_is_set() {
    let first = NotificationStreamId::try_new("first").expect("id");
    let second = NotificationStreamId::try_new("second").expect("id");
    let mut plan = RoutePlan::default();
    assert!(plan.is_empty());
    plan.merge(RoutePlan {
        recipients: Vec::new(),
        stream: Some(first.clone()),
    });
    plan.merge(RoutePlan {
        recipients: Vec::new(),
        stream: Some(second),
    });
    assert_eq!(plan.stream, Some(first));
}
