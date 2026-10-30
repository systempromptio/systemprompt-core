use chrono::Utc;
use systemprompt_extension::prelude::ChangeSource;
use systemprompt_identifiers::DocumentId;
use systemprompt_models::notifications::{
    Attention, AudienceSubject, NotificationEvent, NotificationKind, RuleAction, RuleMatch,
    StreamAttention,
};
use systemprompt_notifications::NotificationsError;
use systemprompt_notifications::service::PublishRequest;

use crate::harness::{Harness, profile, share, stream_subscription};

fn sales() -> AudienceSubject {
    AudienceSubject::Role("sales".to_owned())
}

fn item() -> PublishRequest {
    PublishRequest {
        title: "Quarter closed".to_owned(),
        body: Some("Numbers are in the shared folder.".to_owned()),
        subjects: Vec::new(),
        attention: None,
    }
}

#[tokio::test]
async fn subscribing_to_a_stream_outside_its_audience_is_refused_not_in_audience() {
    let h = Harness::new("notif_subscribe_gate").await;
    let lead = h.user("lead", &["sales_lead"]).await;
    let member = h.user("member", &["sales"]).await;
    let outsider = h.user("outsider", &["support"]).await;
    let stream = h
        .declare_stream(
            "gated-news",
            vec![sales()],
            vec![AudienceSubject::User(lead)],
            StreamAttention::default(),
        )
        .await;

    let refused = h
        .service
        .subscribe(
            &outsider,
            stream_subscription(AudienceSubject::User(outsider.clone()), &stream),
        )
        .await
        .expect_err("an outsider may not subscribe");
    assert!(
        matches!(&refused, NotificationsError::NotInAudience { stream: s } if s == &stream),
        "{refused:?}"
    );
    assert!(
        h.service
            .subscriptions(&outsider)
            .await
            .expect("list")
            .is_empty(),
        "a refused subscription stores nothing"
    );

    let admitted = h
        .service
        .subscribe(
            &member,
            stream_subscription(AudienceSubject::User(member.clone()), &stream),
        )
        .await
        .expect("a member may subscribe");
    assert_eq!(admitted.source, ChangeSource::Console(member.clone()));
    h.finish().await;
}

#[tokio::test]
async fn a_declared_subscription_for_a_non_member_is_withheld_with_a_not_in_audience_failure() {
    let h = Harness::new("notif_stale_subscription").await;
    let lead = h.user("lead", &["sales_lead"]).await;
    let member = h.user("member", &["sales"]).await;
    let outsider = h.user("outsider", &["support"]).await;
    let stream = h
        .declare_stream(
            "gated-news",
            vec![sales()],
            vec![AudienceSubject::User(lead.clone())],
            StreamAttention::default(),
        )
        .await;
    h.declare_subscription(&stream_subscription(
        AudienceSubject::User(outsider.clone()),
        &stream,
    ))
    .await;

    h.service
        .publish(&lead, &stream, item())
        .await
        .expect("publish");
    h.drain().await;

    assert!(
        h.rows_for(&outsider).await.is_empty(),
        "a subscription never widens the audience"
    );
    assert_eq!(
        h.failures("not_in_audience").await,
        vec![Some(outsider.as_str().to_owned())]
    );
    let rows = h.rows_for(&member).await;
    assert_eq!(rows.len(), 1);
    assert!(
        rows[0].title.starts_with("Quarter closed"),
        "a reader who passed the audience gate sees the title"
    );
    h.finish().await;
}

#[tokio::test]
async fn a_subscriber_no_resolver_can_place_is_withheld_audience_unknown() {
    let h = Harness::new("notif_audience_unknown").await;
    let lead = h.user("lead", &[]).await;
    let subscriber = h.user("subscriber", &[]).await;
    let stream = h
        .declare_stream(
            "dimension-news",
            vec![AudienceSubject::Dimension {
                kind: "region".to_owned(),
                value: "emea".to_owned(),
            }],
            vec![AudienceSubject::User(lead.clone())],
            StreamAttention::default(),
        )
        .await;
    h.declare_subscription(&stream_subscription(
        AudienceSubject::User(subscriber.clone()),
        &stream,
    ))
    .await;

    h.service
        .publish(&lead, &stream, item())
        .await
        .expect("publish");
    h.drain().await;

    assert!(
        h.rows_for(&subscriber).await.is_empty(),
        "unknown fails closed"
    );
    assert_eq!(
        h.failures("audience_unknown").await,
        vec![Some(subscriber.as_str().to_owned())]
    );
    h.finish().await;
}

#[tokio::test]
async fn a_subject_less_stream_item_withholds_its_content_from_a_reader_outside_the_audience() {
    let h = Harness::new("notif_content_gate").await;
    let lead = h.user("lead", &["sales_lead"]).await;
    let member = h.user("member", &["sales"]).await;
    let outsider = h.user("outsider", &["support"]).await;
    let stream = h
        .declare_stream(
            "gated-news",
            vec![sales()],
            vec![AudienceSubject::User(lead.clone())],
            StreamAttention::default(),
        )
        .await;
    let event = NotificationEvent::new(
        NotificationKind::new("deal.stage_changed").expect("kind"),
        lead.clone(),
        Utc::now(),
    )
    .with_stream(stream)
    .with_direct_recipient(outsider.clone(), Some(Attention::Agent))
    .with_direct_recipient(member.clone(), Some(Attention::Agent))
    .with_title("confidential pipeline figures")
    .with_body("q3 numbers");

    h.emit(&event).await;
    h.drain().await;

    let outside = h.rows_for(&outsider).await;
    assert_eq!(outside.len(), 1);
    assert_eq!(
        (outside[0].title.as_str(), outside[0].body.as_str()),
        ("", ""),
        "the title and body are denied unless the reader passed the gate"
    );
    let inside = h.rows_for(&member).await;
    assert_eq!(inside.len(), 1);
    assert_eq!(inside[0].title, "confidential pipeline figures");
    assert_eq!(inside[0].body, "q3 numbers");
    h.finish().await;
}

fn small() -> systemprompt_manifest::profile::NotificationsProfile {
    systemprompt_manifest::profile::NotificationsProfile {
        max_audience: 2,
        ..profile()
    }
}

#[tokio::test]
async fn a_role_over_max_audience_is_withheld_whole_with_one_over_limit_failure() {
    let h = Harness::with_profile("notif_over_limit", small()).await;
    let lead = h.user("lead", &[]).await;
    let mut members = Vec::new();
    for name in ["one", "two", "three"] {
        members.push(h.user(name, &["big"]).await);
    }
    let stream = h
        .declare_stream(
            "big-news",
            vec![AudienceSubject::Role("big".to_owned())],
            vec![AudienceSubject::User(lead.clone())],
            StreamAttention::default(),
        )
        .await;

    h.service
        .publish(&lead, &stream, item())
        .await
        .expect("publish");
    let drained = h.drain().await;

    assert_eq!(drained.persisted(), 0, "never a truncated audience");
    for member in &members {
        assert!(h.rows_for(member).await.is_empty());
    }
    assert_eq!(
        h.failures("audience_over_limit").await,
        vec![None],
        "the subject is recorded once, not once per member"
    );
    h.finish().await;
}

#[tokio::test]
async fn a_rule_over_a_role_past_max_audience_never_raises_attention() {
    let h = Harness::with_profile("notif_rule_over_limit", small()).await;
    let owner = h.user("owner", &[]).await;
    let mut members = Vec::new();
    for name in ["one", "two", "three"] {
        members.push(h.user(name, &["big"]).await);
    }
    h.declare_rule(
        RuleMatch {
            recipients: vec![AudienceSubject::Role("big".to_owned())],
            ..RuleMatch::default()
        },
        RuleAction {
            floor: Some(Attention::Human),
            ..RuleAction::default()
        },
    )
    .await;

    for member in &members {
        h.emit(&share(&owner, member, &DocumentId::generate()))
            .await;
    }
    h.drain().await;

    for member in &members {
        let rows = h.rows_for(member).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].attention,
            Attention::Agent,
            "an unprovable membership may lower but never raise"
        );
    }
    assert!(!h.failures("audience_over_limit").await.is_empty());
    h.finish().await;
}
