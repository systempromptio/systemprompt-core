use chrono::Utc;
use systemprompt_identifiers::{DocumentId, NotificationId, UserId};
use systemprompt_models::notifications::{ActionPrompt, NotificationKind};
use systemprompt_notifications::repository::UNREAD_CAP;
use systemprompt_notifications::service::FeedQuery;

use crate::harness::{Harness, mention};

#[tokio::test]
async fn seen_twice_writes_one_receipt_and_only_for_the_readers_own_rows() {
    let h = Harness::new("notif_seen_once").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let (event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    h.emit(&event).await;
    h.drain().await;
    let row = h.rows_for(&reader).await.remove(0);
    let ids = vec![row.id.clone(), NotificationId::generate()];

    assert_eq!(h.service.seen(&reader, &ids).await.expect("seen"), 1);
    assert_eq!(h.service.seen(&reader, &ids).await.expect("seen"), 0);
    assert_eq!(h.receipts("seen").await, 1, "a receipt is recorded once");

    assert_eq!(
        h.service.seen(&author, &ids).await.expect("seen"),
        0,
        "another user's row is never marked seen"
    );
    assert_eq!(h.receipts("seen").await, 1);
    h.finish().await;
}

async fn all_receipts(h: &Harness) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM notification_receipts")
        .fetch_one(h.pool.write_pool().as_ref())
        .await
        .expect("receipts")
}

#[tokio::test]
async fn reading_the_feed_records_no_receipt() {
    let h = Harness::new("notif_feed_read_only").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let (event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    h.emit(&event).await;
    h.drain().await;
    let before = all_receipts(&h).await;

    for _ in 0..2 {
        let page = h
            .service
            .feed(&reader, FeedQuery::default())
            .await
            .expect("feed");
        assert_eq!(page.notifications.len(), 1);
    }

    assert_eq!(all_receipts(&h).await, before, "a feed read writes nothing");
    assert_eq!(h.receipts("seen").await, 0);
    h.finish().await;
}

async fn insert_unread(h: &Harness, user: &UserId, count: i64) {
    let action = serde_json::to_value(ActionPrompt {
        prompt: "bulk".to_owned(),
        open_url: None,
        document_id: None,
        comment_id: None,
        kind: NotificationKind::new(NotificationKind::SHARED).expect("kind"),
        actor: user.clone(),
    })
    .expect("action");
    sqlx::query(
        "INSERT INTO notifications (recipient_user_id, kind, actor_user_id, actor_is_agent, \
         subjects, attention, attention_reason, action, coalesce_key, first_event_at, \
         last_event_at, outbox_id, delivery_state) \
         SELECT $1, 'document.shared', $1, FALSE, '[]'::jsonb, 'silent', 'default', $2, \
         'bulk:' || gen_random_uuid()::text, now(), now(), 'bulk', 'delivered' \
         FROM generate_series(1, $3)",
    )
    .bind(user.as_str())
    .bind(action)
    .bind(count)
    .execute(h.pool.write_pool().as_ref())
    .await
    .expect("bulk insert");
}

#[tokio::test]
async fn the_unread_count_stops_at_the_cap_and_says_it_is_capped() {
    let h = Harness::new("notif_unread_cap").await;
    let reader = h.user("reader", &[]).await;

    insert_unread(&h, &reader, UNREAD_CAP).await;
    let page = h
        .service
        .feed(&reader, FeedQuery::default())
        .await
        .expect("feed");
    assert_eq!(page.unread, UNREAD_CAP);
    assert!(!page.unread_capped, "exactly the cap is an exact count");

    insert_unread(&h, &reader, 1).await;
    let page = h
        .service
        .feed(&reader, FeedQuery::default())
        .await
        .expect("feed");
    assert_eq!(page.unread, UNREAD_CAP);
    assert!(page.unread_capped, "past the cap the count says so");
    assert_eq!(
        h.service.unread_count(&reader).await.expect("count"),
        UNREAD_CAP
    );
    h.finish().await;
}
