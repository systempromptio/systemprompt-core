use chrono::NaiveTime;
use systemprompt_identifiers::UserId;
use systemprompt_models::notifications::{
    Attention, AttentionReason, AudienceSubject, QuietHours, RuleAction, RuleMatch,
    StreamAttention, SubscriptionDecl,
};
use systemprompt_notifications::service::PublishRequest;

use crate::harness::{Harness, stream_subscription};

struct Fixture {
    h: Harness,
    lead: UserId,
    member: UserId,
    stream: systemprompt_identifiers::NotificationStreamId,
}

async fn fixture(prefix: &str, attention: StreamAttention) -> Fixture {
    let h = Harness::new(prefix).await;
    let lead = h.user("lead", &["sales_lead"]).await;
    let member = h.user("member", &["sales"]).await;
    let stream = h
        .declare_stream(
            "policy-news",
            vec![AudienceSubject::Role("sales".to_owned())],
            vec![AudienceSubject::User(lead.clone())],
            attention,
        )
        .await;
    Fixture {
        h,
        lead,
        member,
        stream,
    }
}

impl Fixture {
    async fn publish(&self) -> (Attention, AttentionReason) {
        self.h
            .service
            .publish(
                &self.lead,
                &self.stream,
                PublishRequest {
                    title: "Policy order".to_owned(),
                    body: None,
                    subjects: Vec::new(),
                    attention: None,
                },
            )
            .await
            .expect("publish");
        self.h.drain().await;
        let rows = self.h.rows_for(&self.member).await;
        assert_eq!(rows.len(), 1, "{rows:?}");
        (rows[0].attention, rows[0].attention_reason.clone())
    }

    async fn subscribe(&self, decl: SubscriptionDecl) {
        self.h
            .service
            .subscribe(&self.member, decl)
            .await
            .expect("member subscription");
    }
}

fn always_quiet() -> QuietHours {
    QuietHours {
        start: NaiveTime::from_hms_opt(0, 0, 0).expect("time"),
        end: NaiveTime::from_hms_opt(0, 0, 0).expect("time"),
        tz: "Mars/Olympus_Mons".to_owned(),
        weekdays: None,
    }
}

#[tokio::test]
async fn a_recipient_cap_lowers_below_the_stream_floor_because_the_floor_applies_first() {
    let f = fixture(
        "notif_policy_floor_first",
        StreamAttention {
            default: Attention::Silent,
            floor: Some(Attention::Agent),
            ceiling: None,
        },
    )
    .await;
    let cap =
        f.h.declare_rule(
            RuleMatch::default(),
            RuleAction {
                cap: Some(Attention::Silent),
                ..RuleAction::default()
            },
        )
        .await;

    assert_eq!(
        f.publish().await,
        (Attention::Silent, AttentionReason::Rule(cap))
    );
    f.h.finish().await;
}

#[tokio::test]
async fn a_subscription_ceiling_applies_after_the_rules_and_is_the_stored_reason() {
    let f = fixture(
        "notif_policy_ceiling",
        StreamAttention {
            default: Attention::Agent,
            floor: None,
            ceiling: None,
        },
    )
    .await;
    f.h.declare_rule(
        RuleMatch::default(),
        RuleAction {
            floor: Some(Attention::Human),
            ..RuleAction::default()
        },
    )
    .await;
    f.subscribe(SubscriptionDecl {
        attention_ceiling: Some(Attention::Silent),
        ..stream_subscription(AudienceSubject::User(f.member.clone()), &f.stream)
    })
    .await;

    assert_eq!(
        f.publish().await,
        (Attention::Silent, AttentionReason::SubscriptionCeiling)
    );
    f.h.finish().await;
}

#[tokio::test]
async fn quiet_hours_apply_last_and_are_the_stored_reason() {
    let f = fixture(
        "notif_policy_quiet",
        StreamAttention {
            default: Attention::Human,
            floor: None,
            ceiling: None,
        },
    )
    .await;
    f.subscribe(SubscriptionDecl {
        quiet_hours: Some(always_quiet()),
        ..stream_subscription(AudienceSubject::User(f.member.clone()), &f.stream)
    })
    .await;

    assert_eq!(
        f.publish().await,
        (Attention::Agent, AttentionReason::QuietHours)
    );
    f.h.finish().await;
}

#[tokio::test]
async fn a_step_that_changes_nothing_keeps_the_earlier_reason() {
    let f = fixture(
        "notif_policy_noop",
        StreamAttention {
            default: Attention::Silent,
            floor: Some(Attention::Agent),
            ceiling: None,
        },
    )
    .await;
    f.subscribe(SubscriptionDecl {
        attention_ceiling: Some(Attention::Human),
        ..stream_subscription(AudienceSubject::User(f.member.clone()), &f.stream)
    })
    .await;

    assert_eq!(
        f.publish().await,
        (
            Attention::Agent,
            AttentionReason::StreamFloor(f.stream.clone())
        )
    );
    f.h.finish().await;
}
