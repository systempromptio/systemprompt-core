use systemprompt_identifiers::{NotificationRuleId, NotificationStreamId};
use systemprompt_models::notifications::{Attention, AttentionReason};

#[test]
fn attention_orders_silent_below_agent_below_human() {
    assert!(Attention::Silent < Attention::Agent);
    assert!(Attention::Agent < Attention::Human);
    assert_eq!(Attention::default(), Attention::Silent);
    assert_eq!(Attention::Silent.max(Attention::Human), Attention::Human);
}

#[test]
fn attention_string_forms_round_trip() {
    for level in [Attention::Silent, Attention::Agent, Attention::Human] {
        let text = level.to_string();
        assert_eq!(text, level.as_str());
        assert_eq!(text.parse::<Attention>().expect("parses"), level);
        let json = serde_json::to_string(&level).expect("serialises");
        assert_eq!(json, format!("\"{text}\""));
        assert_eq!(
            serde_json::from_str::<Attention>(&json).expect("deserialises"),
            level
        );
    }
}

#[test]
fn an_unknown_attention_is_refused() {
    let error = "loud".parse::<Attention>().expect_err("not a level");
    assert_eq!(error.field, "attention");
    assert_eq!(error.value, "loud");
    assert!("Human".parse::<Attention>().is_err(), "case matters");
}

#[test]
fn attention_reasons_round_trip_through_their_string_form() {
    let rule = NotificationRuleId::generate();
    let stream = NotificationStreamId::try_new("sales-news").expect("stream id");
    let reasons = [
        AttentionReason::Sender,
        AttentionReason::Default,
        AttentionReason::Escalated,
        AttentionReason::Rule(rule.clone()),
        AttentionReason::StreamFloor(stream.clone()),
        AttentionReason::StreamCeiling(stream),
        AttentionReason::SubscriptionCeiling,
        AttentionReason::QuietHours,
    ];
    for reason in reasons {
        let text = reason.to_string();
        assert_eq!(text.parse::<AttentionReason>().expect("parses"), reason);
        let json = serde_json::to_value(&reason).expect("serialises");
        assert_eq!(json, serde_json::Value::String(text));
        assert_eq!(
            serde_json::from_value::<AttentionReason>(json).expect("deserialises"),
            reason
        );
    }
    assert_eq!(
        AttentionReason::Rule(rule.clone()).to_string(),
        format!("rule:{rule}")
    );
    assert_eq!(
        AttentionReason::SubscriptionCeiling.to_string(),
        "subscription_ceiling"
    );
    assert_eq!(AttentionReason::QuietHours.to_string(), "quiet_hours");
}

#[test]
fn a_reason_with_a_malformed_id_or_tag_is_refused() {
    assert!("rule:not-a-uuid".parse::<AttentionReason>().is_err());
    assert!(
        "stream_floor:Upper Case"
            .parse::<AttentionReason>()
            .is_err()
    );
    assert!("whim".parse::<AttentionReason>().is_err());
    assert!(
        "policy:5c1b3c38-8e3c-4b8e-9a51-3b2d3f1d9a10"
            .parse::<AttentionReason>()
            .is_err()
    );
    let error = "rule:nope"
        .parse::<AttentionReason>()
        .expect_err("invalid id");
    assert!(error.source.is_some(), "an invalid id keeps its cause");
}
