use chrono::Utc;
use systemprompt_identifiers::{DocumentId, NotificationId};
use systemprompt_models::notifications::{
    Attention, AttentionReason, DeliveryState, Escalate, SURFACE_API,
};
use systemprompt_notifications::NotificationsError;

use crate::harness::{Harness, mention, share};

#[tokio::test]
async fn an_agent_notification_escalates_to_human_once() {
    let h = Harness::new("notif_escalate").await;
    let owner = h.user("owner", &[]).await;
    let reader = h.user("reader", &[]).await;
    h.emit(&share(&owner, &reader, &DocumentId::generate()))
        .await;
    h.drain().await;
    let row = h.rows_for(&reader).await.remove(0);
    assert_eq!(row.attention, Attention::Agent, "a share defaults to agent");

    let escalated = h
        .service
        .escalate(
            &reader,
            &row.id,
            Escalate {
                note: Some("needs a decision today".to_owned()),
            },
            SURFACE_API,
        )
        .await
        .expect("escalate");
    assert_eq!(escalated.attention, Attention::Human);
    assert_eq!(escalated.escalated_from, Some(Attention::Agent));
    assert_eq!(escalated.attention_reason, AttentionReason::Escalated);
    assert_eq!(
        escalated.escalation_note.as_deref(),
        Some("needs a decision today")
    );
    assert_eq!(
        h.rows_for(&reader).await[0].delivery_state,
        DeliveryState::Delivered,
        "the escalated row settles delivered"
    );
    assert_eq!(
        h.channel.deliveries().len(),
        1,
        "a surface that already announced the row is not called again"
    );

    let again = h
        .service
        .escalate(&reader, &row.id, Escalate::default(), SURFACE_API)
        .await
        .expect_err("a human notification is not escalatable");
    assert!(
        matches!(
            again,
            NotificationsError::NotEscalatable {
                attention: Attention::Human,
                ..
            }
        ),
        "{again:?}"
    );
    h.finish().await;
}

#[tokio::test]
async fn a_human_mention_cannot_be_escalated_and_a_foreign_id_is_not_found() {
    let h = Harness::new("notif_escalate_refused").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let (event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    h.emit(&event).await;
    h.drain().await;
    let row = h.rows_for(&reader).await.remove(0);

    let refused = h
        .service
        .escalate(&reader, &row.id, Escalate::default(), SURFACE_API)
        .await
        .expect_err("already human");
    assert!(
        matches!(refused, NotificationsError::NotEscalatable { .. }),
        "{refused:?}"
    );

    let foreign = h
        .service
        .escalate(&author, &row.id, Escalate::default(), SURFACE_API)
        .await
        .expect_err("not the author's notification");
    assert!(
        matches!(foreign, NotificationsError::NotFound { .. }),
        "{foreign:?}"
    );

    let missing = h
        .service
        .escalate(
            &reader,
            &NotificationId::generate(),
            Escalate::default(),
            SURFACE_API,
        )
        .await
        .expect_err("no such notification");
    assert!(
        matches!(missing, NotificationsError::NotFound { .. }),
        "{missing:?}"
    );
    h.finish().await;
}

#[derive(Default)]
struct Pager {
    paged: std::sync::Mutex<Vec<NotificationId>>,
}

#[async_trait::async_trait]
impl systemprompt_extension::notifications::NotificationChannel for Pager {
    fn surface(&self) -> &'static str {
        "pager"
    }

    fn delivers(&self, attention: Attention) -> bool {
        attention == Attention::Human
    }

    async fn deliver(
        &self,
        notification: &systemprompt_models::notifications::Notification,
        _unread: i64,
    ) -> Result<(), systemprompt_extension::notifications::ChannelError> {
        self.paged
            .lock()
            .expect("lock")
            .push(notification.id.clone());
        Ok(())
    }
}

#[tokio::test]
async fn an_escalation_from_a_core_only_process_is_delivered_by_the_consumer_with_its_extension_channel()
 {
    use std::sync::Arc;

    use systemprompt_events::{EventBroadcasters, EventRouter};
    use systemprompt_notifications::pipeline::{NotificationPipeline, PipelineParts};
    use systemprompt_notifications::service::{
        EscalationDelivery, NotificationsService, NotificationsServiceParts,
    };

    let pager = Arc::new(Pager::default());
    let paging = Arc::clone(&pager);
    let h = Harness::with_seams(
        "notif_escalate_host",
        crate::harness::profile(),
        move |parts| {
            parts.channels.push(paging);
        },
    )
    .await;
    let owner = h.user("owner", &[]).await;
    let reader = h.user("reader", &[]).await;
    h.emit(&share(&owner, &reader, &DocumentId::generate()))
        .await;
    h.drain().await;
    let row = h.rows_for(&reader).await.remove(0);
    assert!(pager.paged.lock().expect("lock").is_empty());

    let core_only = NotificationPipeline::new(PipelineParts {
        routers: Vec::new(),
        audiences: Vec::new(),
        visibilities: Vec::new(),
        channels: Vec::new(),
        repos: Arc::clone(&h.repos),
        profile: crate::harness::profile(),
    });
    let host = NotificationsService::new(NotificationsServiceParts {
        repos: Arc::clone(&h.repos),
        router: Arc::new(EventRouter::local_only(Arc::new(EventBroadcasters::new()))),
        sink: Arc::clone(&h.sink),
        pipeline: Arc::new(core_only),
        publish_policies: Vec::new(),
        db: Arc::clone(&h.pool),
        escalation: EscalationDelivery::Consumer,
    });
    host.escalate(&reader, &row.id, Escalate::default(), SURFACE_API)
        .await
        .expect("escalate from the host");
    assert_eq!(
        h.rows_for(&reader).await[0].delivery_state,
        DeliveryState::Pending,
        "the host leaves the escalated row for the consumer"
    );
    assert!(
        pager.paged.lock().expect("lock").is_empty(),
        "the host delivers through no channel"
    );

    let grace = chrono::Duration::seconds(i64::from(crate::harness::profile().consumer.sweep_secs));
    h.pipeline
        .sweep(Utc::now() + grace + grace)
        .await
        .expect("sweep");
    assert_eq!(
        pager.paged.lock().expect("lock").as_slice(),
        std::slice::from_ref(&row.id),
        "the consumer delivers the escalation through the extension channel"
    );
    assert_eq!(
        h.rows_for(&reader).await[0].delivery_state,
        DeliveryState::Delivered
    );
    h.finish().await;
}
