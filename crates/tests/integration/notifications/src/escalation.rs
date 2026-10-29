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
