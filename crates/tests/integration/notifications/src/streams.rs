use systemprompt_identifiers::NotificationStreamId;
use systemprompt_models::notifications::{
    Attention, AttentionReason, AudienceSubject, StreamAttention, StreamDecl, SubscriptionDecl,
    SubscriptionTarget,
};
use systemprompt_notifications::NotificationsError;
use systemprompt_notifications::repository::{
    NotificationStreamRepository, NotificationSubscriptionRepository,
};
use systemprompt_notifications::service::PublishRequest;

use crate::harness::Harness;

fn announcement() -> PublishRequest {
    PublishRequest {
        title: "Quarter closed".to_owned(),
        body: Some("Numbers are in the shared folder.".to_owned()),
        subjects: Vec::new(),
        attention: None,
    }
}

async fn declare(h: &Harness, attention: StreamAttention) -> NotificationStreamId {
    let id = NotificationStreamId::try_new("sales-news").expect("stream id");
    let stream = StreamDecl {
        id: id.clone(),
        name: "Sales news".to_owned(),
        description: None,
        audience: vec![AudienceSubject::Role("sales".to_owned())],
        publishers: vec![AudienceSubject::Role("sales_lead".to_owned())],
        attention,
        retention_days: None,
        coalesce_window_secs: None,
    };
    NotificationStreamRepository::upsert(h.repos.write_pool(), &stream, "test")
        .await
        .expect("declare stream");
    let subscription = SubscriptionDecl {
        subject: AudienceSubject::Role("sales".to_owned()),
        target: SubscriptionTarget::Stream(id.clone()),
        attention_ceiling: None,
        quiet_hours: None,
        digest: None,
    };
    NotificationSubscriptionRepository::upsert(h.repos.write_pool(), &subscription, "test", None)
        .await
        .expect("subscribe role");
    id
}

#[tokio::test]
async fn only_a_declared_publisher_may_publish_and_role_subscribers_receive_silently() {
    let h = Harness::new("notif_streams").await;
    let lead = h.user("lead", &["sales_lead"]).await;
    let first = h.user("first", &["sales"]).await;
    let second = h.user("second", &["sales"]).await;
    let outsider = h.user("outsider", &["support"]).await;
    let stream = declare(&h, StreamAttention::default()).await;

    let refused = h
        .service
        .publish(&first, &stream, announcement())
        .await
        .expect_err("a subscriber is not a publisher");
    assert!(
        matches!(refused, NotificationsError::PublishRefused { .. }),
        "{refused:?}"
    );

    let outcome = h
        .service
        .publish(&lead, &stream, announcement())
        .await
        .expect("publish");
    assert!(outcome.enqueued);
    let drained = h.drain().await;
    assert_eq!(drained.ids.len(), 1, "the refused publish enqueued nothing");

    for member in [&first, &second] {
        let rows = h.rows_for(member).await;
        assert_eq!(rows.len(), 1, "every role member gets one row");
        assert_eq!(rows[0].attention, Attention::Silent);
        assert_eq!(rows[0].attention_reason, AttentionReason::Default);
        assert_eq!(rows[0].stream.as_ref(), Some(&stream));
        assert!(
            rows[0]
                .excerpt
                .as_deref()
                .is_some_and(|e| e.starts_with("Quarter closed"))
        );
    }
    assert!(
        h.rows_for(&lead).await.is_empty(),
        "the publisher is not notified"
    );
    assert!(h.rows_for(&outsider).await.is_empty());
    h.finish().await;
}

#[tokio::test]
async fn a_stream_floor_lifts_its_audience_and_names_the_stream_as_the_reason() {
    let h = Harness::new("notif_stream_floor").await;
    let lead = h.user("lead", &["sales_lead"]).await;
    let member = h.user("member", &["sales"]).await;
    let stream = declare(
        &h,
        StreamAttention {
            default: Attention::Silent,
            floor: Some(Attention::Agent),
            ceiling: None,
        },
    )
    .await;

    h.service
        .publish(&lead, &stream, announcement())
        .await
        .expect("publish");
    h.drain().await;

    let rows = h.rows_for(&member).await;
    assert_eq!(rows[0].attention, Attention::Agent);
    assert_eq!(
        rows[0].attention_reason,
        AttentionReason::StreamFloor(stream)
    );
    h.finish().await;
}

#[tokio::test]
async fn publishing_to_an_undeclared_stream_is_not_found() {
    let h = Harness::new("notif_stream_missing").await;
    let lead = h.user("lead", &["sales_lead"]).await;
    let missing = NotificationStreamId::try_new("nowhere").expect("stream id");
    let error = h
        .service
        .publish(&lead, &missing, announcement())
        .await
        .expect_err("no such stream");
    assert!(
        matches!(error, NotificationsError::StreamNotFound { .. }),
        "{error:?}"
    );
    h.finish().await;
}
