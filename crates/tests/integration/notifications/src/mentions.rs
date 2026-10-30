use chrono::Utc;
use systemprompt_identifiers::DocumentId;
use systemprompt_models::notifications::{
    Attention, AttentionReason, DeliveryState, MarkRead, NOTIFICATIONS_CONSUMER, SURFACE_API,
};
use systemprompt_notifications::service::FeedQuery;

use crate::harness::{Harness, mention};

#[tokio::test]
async fn a_mention_flows_from_the_outbox_to_the_feed_and_read_state() {
    let h = Harness::new("notif_mention").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let document = DocumentId::generate();
    let (event, comment) = mention(&author, &reader, &document, Utc::now());

    h.emit(&event).await;
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM event_outbox WHERE consumer = $1 AND channel = 'fact' \
             AND processed_at IS NULL",
            NOTIFICATIONS_CONSUMER,
        )
        .await,
        1,
        "the mention is one pending fact-only outbox row for the notifications consumer"
    );

    let drained = h.drain().await;
    assert_eq!(drained.persisted(), 1);
    assert_eq!(drained.delivered.delivered, 1);
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM event_outbox WHERE consumer = $1 AND processed_at IS NULL",
            NOTIFICATIONS_CONSUMER,
        )
        .await,
        0,
        "the drained fact is acknowledged"
    );

    let rows = h.rows_for(&reader).await;
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.attention, Attention::Human);
    assert_eq!(row.attention_reason, AttentionReason::Default);
    assert_eq!(row.actor, author);
    assert_eq!(row.coalesce_count, 1);
    assert_eq!(row.held_until, None);
    assert_eq!(row.delivery_state, DeliveryState::Delivered);
    assert_eq!(row.body, "please review the pricing table");
    assert_eq!(row.title, "");
    assert!(!row.action.prompt.is_empty());
    assert_eq!(
        row.action.open_url.as_deref(),
        Some(format!("/admin/drive/{document}?comment={comment}").as_str())
    );
    assert!(
        h.rows_for(&author).await.is_empty(),
        "the actor is never a recipient"
    );
    let deliveries = h.channel.deliveries();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].0, row.id);
    assert_eq!(deliveries[0].2, 1, "the channel is told the unread count");

    let page = h
        .service
        .feed(
            &reader,
            FeedQuery {
                limit: 1,
                ..FeedQuery::default()
            },
        )
        .await
        .expect("feed");
    assert_eq!(page.unread, 1);
    assert!(!page.unread_capped);
    assert_eq!(page.notifications.len(), 1);
    let cursor = page.next_cursor.expect("a full page carries a cursor");
    let rest = h
        .service
        .feed(
            &reader,
            FeedQuery {
                since: Some(cursor),
                limit: 1,
                ..FeedQuery::default()
            },
        )
        .await
        .expect("next page");
    assert!(rest.notifications.is_empty());
    assert_eq!(rest.next_cursor, None, "a short page carries no cursor");
    assert_eq!(
        h.service
            .seen(&reader, std::slice::from_ref(&row.id))
            .await
            .expect("seen"),
        1
    );

    let read = h
        .service
        .mark_read(
            &reader,
            MarkRead {
                ids: vec![row.id.clone()],
                all: false,
            },
            SURFACE_API,
        )
        .await
        .expect("mark read");
    assert_eq!(read.ids, vec![row.id.clone()]);
    assert_eq!(read.unread, 0);
    assert_eq!(h.service.unread_count(&reader).await.expect("count"), 0);

    for state in ["delivered", "seen", "read"] {
        let receipts: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM notification_receipts WHERE notification_id = $1::uuid \
             AND state = $2",
        )
        .bind(row.id.as_str())
        .bind(state)
        .fetch_one(h.pool.write_pool().as_ref())
        .await
        .expect("receipts");
        assert_eq!(receipts, 1, "one {state} receipt");
    }
    h.finish().await;
}

#[tokio::test]
async fn marking_another_users_notification_read_changes_nothing() {
    let h = Harness::new("notif_foreign_read").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let (event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    h.emit(&event).await;
    h.drain().await;
    let row = h.rows_for(&reader).await.remove(0);

    let outcome = h
        .service
        .mark_read(
            &author,
            MarkRead {
                ids: vec![row.id.clone()],
                all: false,
            },
            SURFACE_API,
        )
        .await
        .expect("mark read");
    assert!(outcome.ids.is_empty());
    assert_eq!(h.service.unread_count(&reader).await.expect("count"), 1);
    h.finish().await;
}

#[tokio::test]
async fn processing_the_same_outbox_fact_twice_yields_one_row() {
    let h = Harness::new("notif_idempotent").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let (event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    h.emit(&event).await;
    let drained = h.drain().await;
    assert_eq!(drained.persisted(), 1);

    let again = h.reprocess(&drained.ids[0], &event).await;
    assert!(again.persisted.is_empty(), "{again:?}");
    assert!(again.pending.is_empty());
    let rows = h.rows_for(&reader).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].coalesce_count, 1);
    h.finish().await;
}
