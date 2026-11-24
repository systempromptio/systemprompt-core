//! Per-aggregate claim order: while an earlier row of an aggregate is
//! claimed or pending, no later row of that aggregate is handed out, and rows
//! of other aggregates and unkeyed rows still are.

use systemprompt_events::services::durable::{
    DurableOutbox, OutboxAppend, OutboxConsumer, ReportingFact, SseEvent,
};
use systemprompt_identifiers::{Actor, ConnectionId, EventOutboxId, OutboxAggregateKey, UserId};
use systemprompt_test_fixtures::unique_instance;

async fn append(
    pool: &sqlx::PgPool,
    consumer: &str,
    aggregate: Option<&OutboxAggregateKey>,
    value: i64,
) -> EventOutboxId {
    let outbox = DurableOutbox::new(pool.clone(), unique_instance());
    let actor = Actor::user(UserId::new(format!("user_{consumer}")));
    let fact = ReportingFact {
        consumer: consumer.to_owned(),
        kind: "test.claim_order".to_owned(),
        version: 1,
        data: value,
    };
    let mut tx = pool.begin().await.expect("begin");
    let id = match aggregate {
        Some(key) => {
            outbox
                .append_in_order_on(
                    tx.as_mut(),
                    OutboxAppend {
                        actor: &actor,
                        event: SseEvent::FactOnly,
                        fact: &fact,
                    },
                    key,
                )
                .await
        },
        None => {
            outbox
                .append_on(tx.as_mut(), &actor, SseEvent::FactOnly, &fact)
                .await
        },
    }
    .expect("append");
    tx.commit().await.expect("commit");
    id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_later_row_of_an_aggregate_waits_for_the_earlier_one() {
    let scratch = crate::OutboxSchema::create().await;
    let pool = scratch.pool_of(4).await;
    let consumer = format!(
        "order_{}",
        ConnectionId::generate().to_string().replace('-', "_")
    );
    let a = OutboxAggregateKey::new("doc:a");
    let b = OutboxAggregateKey::new("doc:b");
    let a1 = append(&pool, &consumer, Some(&a), 1).await;
    let a2 = append(&pool, &consumer, Some(&a), 2).await;
    let b1 = append(&pool, &consumer, Some(&b), 3).await;
    let free = append(&pool, &consumer, None, 4).await;

    let first = OutboxConsumer::new(pool.clone());
    let second = OutboxConsumer::new(pool.clone());

    let held_a1 = first.claim(&consumer).await.expect("claim").expect("a row");
    assert_eq!(held_a1.id(), a1, "the oldest row is claimed first");

    let held_b1 = second
        .claim(&consumer)
        .await
        .expect("claim")
        .expect("a row");
    assert_eq!(held_b1.id(), b1, "a2 waits while a1 is claimed; b1 is free");

    let held_free = second
        .claim(&consumer)
        .await
        .expect("claim")
        .expect("a row");
    assert_eq!(held_free.id(), free, "an unkeyed row is not held back");

    assert!(
        second.claim(&consumer).await.expect("claim").is_none(),
        "no row of aggregate a is handed out while a1 is claimed"
    );

    held_free.acknowledge().await.expect("acknowledge");
    held_b1.acknowledge().await.expect("acknowledge");
    drop(held_a1);

    let again = second
        .claim(&consumer)
        .await
        .expect("claim")
        .expect("a row");
    assert_eq!(again.id(), a1, "a rolled-back claim leaves a1 at the head");
    again.acknowledge().await.expect("acknowledge");

    let next = first.claim(&consumer).await.expect("claim").expect("a row");
    assert_eq!(next.id(), a2, "a2 is handed out once a1 is processed");
    next.acknowledge().await.expect("acknowledge");

    pool.close().await;
    scratch.drop_now().await;
}
