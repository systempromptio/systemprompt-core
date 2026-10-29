use chrono::{Duration, Utc};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::notifications::{DeliveryState, DigestWindow, RuleAction, RuleMatch};

use crate::harness::{Harness, mention};

#[tokio::test]
async fn a_digest_rule_holds_the_row_until_its_window_closes() {
    let h = Harness::new("notif_digest").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    h.declare_rule(
        RuleMatch::default(),
        RuleAction {
            digest: Some(DigestWindow { every_secs: 3600 }),
            ..RuleAction::default()
        },
    )
    .await;
    let (event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    h.emit(&event).await;

    let drained = h.drain().await;
    assert_eq!(drained.outcomes[0].held, 1);
    assert!(drained.outcomes[0].pending.is_empty());
    assert_eq!(drained.delivered.delivered, 0);
    assert!(h.channel.deliveries().is_empty());

    let row = h.rows_for(&reader).await.remove(0);
    assert_eq!(row.delivery_state, DeliveryState::Held);
    let held_until = row.held_until.expect("a digest row is held");
    assert!(held_until > event.occurred_at);
    assert!(held_until <= event.occurred_at + Duration::seconds(3600));
    assert_eq!(
        held_until.timestamp() % 3600,
        0,
        "held to the window boundary"
    );

    let early = h
        .pipeline
        .sweep(held_until - Duration::seconds(1))
        .await
        .expect("sweep");
    assert_eq!(
        early.delivered, 0,
        "nothing is due before the window closes"
    );

    let due = h.pipeline.sweep(held_until).await.expect("sweep");
    assert_eq!(due.delivered, 1);
    assert_eq!(h.channel.deliveries()[0].0, row.id);
    let released = h.rows_for(&reader).await.remove(0);
    assert_eq!(released.held_until, None);
    assert_eq!(released.delivery_state, DeliveryState::Delivered);

    let again = h
        .pipeline
        .sweep(held_until + Duration::seconds(3600))
        .await
        .expect("sweep");
    assert_eq!(again.delivered, 0, "a released row is delivered once");
    assert_eq!(h.channel.deliveries().len(), 1);
    h.finish().await;
}
