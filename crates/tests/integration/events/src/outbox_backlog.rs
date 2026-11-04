//! The `event_outbox_backlog` deltas follow the ledger through its statement
//! triggers: appends raise the sum, acknowledgement and deletion lower it,
//! `compact_event_outbox_backlog` folds the rows to one per consumer,
//! `refresh_event_outbox_backlog` recounts a drifted consumer, and
//! `pending()` answers from the sum plus the oldest row's age. An open
//! transaction that appended never blocks another append for the consumer.

use systemprompt_events::services::durable::{
    DurableOutbox, OutboxConsumer, ReportingFact, SseEvent,
};
use systemprompt_identifiers::{Actor, ConnectionId, EventOutboxId, UserId};
use systemprompt_models::AnalyticsEventBuilder;
use systemprompt_test_fixtures::unique_instance;

fn consumer_name() -> String {
    format!(
        "backlog_{}",
        ConnectionId::generate().to_string().replace('-', "_")
    )
}

async fn append_n(
    pool: &sqlx::PgPool,
    outbox: &DurableOutbox,
    consumer: &str,
    n: i64,
) -> Vec<EventOutboxId> {
    let actor = Actor::user(UserId::new(format!("user_{consumer}")));
    let event = AnalyticsEventBuilder::heartbeat();
    let mut ids = Vec::new();
    for value in 0..n {
        let mut tx = pool.begin().await.expect("begin");
        let fact = ReportingFact {
            consumer: consumer.to_owned(),
            kind: "test.backlog".to_owned(),
            version: 1,
            data: value,
        };
        ids.push(
            outbox
                .append(&mut tx, &actor, SseEvent::Analytics(&event), &fact)
                .await
                .expect("append"),
        );
        tx.commit().await.expect("commit");
    }
    ids
}

async fn counter(pool: &sqlx::PgPool, consumer: &str) -> Option<i64> {
    sqlx::query_scalar("SELECT SUM(delta)::BIGINT FROM event_outbox_backlog WHERE consumer = $1")
        .bind(consumer)
        .fetch_one(pool)
        .await
        .expect("counter")
}

async fn rows(pool: &sqlx::PgPool, consumer: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM event_outbox_backlog WHERE consumer = $1")
        .bind(consumer)
        .fetch_one(pool)
        .await
        .expect("rows")
}

#[tokio::test]
async fn the_counter_follows_insert_acknowledge_and_delete() {
    let pool = crate::setup_test_pool().await;
    let pool = pool.as_ref().clone();
    let consumer = consumer_name();
    let outbox = DurableOutbox::new(pool.clone(), unique_instance());
    let reader = OutboxConsumer::new(pool.clone());
    assert_eq!(counter(&pool, &consumer).await, None);

    let ids = append_n(&pool, &outbox, &consumer, 3).await;
    assert_eq!(counter(&pool, &consumer).await, Some(3));
    let (pending, oldest) = reader.pending(&consumer).await.expect("pending");
    assert_eq!(pending, 3);
    assert!(oldest.is_some_and(|seconds| seconds >= 0));

    let single = reader
        .claim(&consumer)
        .await
        .expect("claim")
        .expect("a pending fact");
    assert_eq!(single.id(), ids[0]);
    single.acknowledge().await.expect("ack");
    assert_eq!(counter(&pool, &consumer).await, Some(2));

    sqlx::query("DELETE FROM event_outbox WHERE id = $1")
        .bind(ids[1].to_string())
        .execute(&pool)
        .await
        .expect("delete pending row");
    assert_eq!(counter(&pool, &consumer).await, Some(1));

    sqlx::query("DELETE FROM event_outbox WHERE id = $1")
        .bind(ids[0].to_string())
        .execute(&pool)
        .await
        .expect("delete processed row");
    assert_eq!(counter(&pool, &consumer).await, Some(1));

    let rest = reader
        .claim(&consumer)
        .await
        .expect("claim")
        .expect("the last fact");
    rest.acknowledge().await.expect("ack");
    assert_eq!(counter(&pool, &consumer).await, Some(0));
    assert_eq!(reader.pending(&consumer).await.expect("pending"), (0, None));
}

#[tokio::test]
async fn the_projector_recounts_a_drifted_consumer() {
    let pool = crate::setup_test_pool().await;
    let pool = pool.as_ref().clone();
    let consumer = consumer_name();
    let outbox = DurableOutbox::new(pool.clone(), unique_instance());
    let reader = OutboxConsumer::new(pool.clone());
    append_n(&pool, &outbox, &consumer, 4).await;

    sqlx::query("INSERT INTO event_outbox_backlog (consumer, delta) VALUES ($1, 95)")
        .bind(&consumer)
        .execute(&pool)
        .await
        .expect("drift");
    assert_eq!(reader.pending(&consumer).await.expect("pending").0, 99);

    let written: i64 = sqlx::query_scalar("SELECT refresh_event_outbox_backlog($1)")
        .bind(vec![consumer.clone()])
        .fetch_one(&pool)
        .await
        .expect("refresh");
    assert_eq!(written, 1);
    assert_eq!(counter(&pool, &consumer).await, Some(4));
    assert_eq!(reader.pending(&consumer).await.expect("pending").0, 4);

    let unknown = consumer_name();
    let written: i64 = sqlx::query_scalar("SELECT refresh_event_outbox_backlog($1)")
        .bind(vec![unknown.clone()])
        .fetch_one(&pool)
        .await
        .expect("refresh");
    assert_eq!(written, 1);
    assert_eq!(counter(&pool, &unknown).await, Some(0));
}

#[tokio::test]
async fn compaction_folds_the_deltas_and_keeps_the_sum() {
    let pool = crate::setup_test_pool().await;
    let pool = pool.as_ref().clone();
    let consumer = consumer_name();
    let outbox = DurableOutbox::new(pool.clone(), unique_instance());
    let reader = OutboxConsumer::new(pool.clone());
    append_n(&pool, &outbox, &consumer, 5).await;
    let two = reader
        .claim_batch(&consumer, 2, &[])
        .await
        .expect("claim")
        .expect("two facts");
    two.acknowledge_all().await.expect("ack");
    assert!(rows(&pool, &consumer).await > 1);
    assert_eq!(counter(&pool, &consumer).await, Some(3));

    sqlx::query("SELECT compact_event_outbox_backlog()")
        .execute(&pool)
        .await
        .expect("compact");
    assert_eq!(rows(&pool, &consumer).await, 1);
    assert_eq!(counter(&pool, &consumer).await, Some(3));
    assert_eq!(reader.pending(&consumer).await.expect("pending").0, 3);
}

#[tokio::test]
async fn an_open_append_does_not_block_another_append_for_the_consumer() {
    let pool = crate::setup_test_pool().await;
    let pool = pool.as_ref().clone();
    let consumer = consumer_name();
    let outbox = DurableOutbox::new(pool.clone(), unique_instance());
    let actor = Actor::user(UserId::new(format!("user_{consumer}")));
    let event = AnalyticsEventBuilder::heartbeat();
    let fact = ReportingFact {
        consumer: consumer.clone(),
        kind: "test.backlog".to_owned(),
        version: 1,
        data: 0_i64,
    };
    let mut open = pool.begin().await.expect("begin");
    outbox
        .append(&mut open, &actor, SseEvent::Analytics(&event), &fact)
        .await
        .expect("append on the open transaction");

    let mut other = pool.begin().await.expect("begin");
    let appended = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        outbox.append(&mut other, &actor, SseEvent::Analytics(&event), &fact),
    )
    .await;
    assert!(appended.is_ok(), "the second append waited on the first");
    other.commit().await.expect("commit");
    open.rollback().await.expect("rollback");
    assert_eq!(counter(&pool, &consumer).await, Some(1));
}
