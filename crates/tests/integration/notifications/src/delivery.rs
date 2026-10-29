use chrono::{Duration, Utc};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::notifications::DeliveryState;

use crate::harness::{Harness, mention};

#[tokio::test]
async fn committed_rows_left_undelivered_are_delivered_exactly_once_by_the_sweep() {
    let h = Harness::new("notif_sweep_pending").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let (event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    h.emit(&event).await;

    let processed = h.process_pending().await;
    assert_eq!(processed.pending.len(), 1);
    assert!(h.channel.deliveries().is_empty());
    let row = h.rows_for(&reader).await.remove(0);
    assert_eq!(row.delivery_state, DeliveryState::Pending);

    let young = h.pipeline.sweep(Utc::now()).await.expect("sweep");
    assert_eq!(
        young.delivered, 0,
        "a row younger than the sweep grace is left to the live path"
    );

    let swept = h
        .pipeline
        .sweep(Utc::now() + Duration::seconds(60))
        .await
        .expect("sweep");
    assert_eq!(swept.delivered, 1);
    let deliveries = h.channel.deliveries();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].0, row.id);
    assert_eq!(
        h.rows_for(&reader).await[0].delivery_state,
        DeliveryState::Delivered
    );

    let again = h
        .pipeline
        .sweep(Utc::now() + Duration::seconds(120))
        .await
        .expect("sweep");
    assert_eq!(again.delivered, 0, "a delivered row is never swept again");
    assert_eq!(h.channel.deliveries().len(), 1);
    assert_eq!(h.receipts("delivered").await, 1);
    h.finish().await;
}

#[tokio::test]
async fn delivering_an_already_delivered_row_announces_nothing() {
    let h = Harness::new("notif_deliver_twice").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let (event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    h.emit(&event).await;
    let drained = h.drain().await;
    assert_eq!(drained.delivered.delivered, 1);

    let replay = h.pipeline.deliver(&drained.pending).await;
    assert_eq!(replay.delivered, 0);
    assert_eq!(h.channel.deliveries().len(), 1);
    h.finish().await;
}
