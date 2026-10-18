//! Batched outbox consumption: a batch claims the oldest unprocessed facts of
//! one consumer, skips ids it is told to, rolls back to redeliver, and an
//! acknowledged batch is never claimed again. Malformed contracts are refused
//! before any row is written.

use systemprompt_events::services::durable::{
    DurableEventError, DurableOutbox, OutboxConsumer, ReportingFact, SseEvent,
};
use systemprompt_identifiers::{Actor, ConnectionId, EventOutboxId, UserId};
use systemprompt_models::AnalyticsEventBuilder;
use systemprompt_test_fixtures::unique_instance;

fn consumer_name() -> String {
    format!(
        "batch_{}",
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
            kind: "test.batch".to_owned(),
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

#[tokio::test]
async fn a_batch_claims_skips_redelivers_and_acknowledges() {
    let pool = crate::setup_test_pool().await;
    let pool = pool.as_ref().clone();
    let consumer = consumer_name();
    let outbox = DurableOutbox::new(pool.clone(), unique_instance());
    let reader = OutboxConsumer::new(pool.clone());
    let ids = append_n(&pool, &outbox, &consumer, 3).await;

    let batch = reader
        .claim_batch(&consumer, 10, &ids[..1])
        .await
        .expect("claim")
        .expect("two unskipped facts");
    assert_eq!(batch.len(), 2);
    assert!(!batch.is_empty());
    let claimed: Vec<&EventOutboxId> = batch.ids().collect();
    assert_eq!(claimed, vec![&ids[1], &ids[2]]);
    let facts = batch.facts::<i64>().expect("facts decode");
    assert_eq!(
        facts.iter().map(|(_, f)| f.data).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert!(
        facts
            .iter()
            .all(|(_, f)| f.kind == "test.batch" && f.consumer == consumer)
    );
    batch.rollback().await.expect("rollback");

    let again = reader
        .claim_batch(&consumer, 2, &[])
        .await
        .expect("claim")
        .expect("rolled-back facts redeliver");
    assert_eq!(again.ids().collect::<Vec<_>>(), vec![&ids[0], &ids[1]]);
    again.acknowledge_all().await.expect("ack");

    let single = reader
        .claim(&consumer)
        .await
        .expect("claim one")
        .expect("third fact still pending");
    assert_eq!(single.id(), ids[2]);
    assert_eq!(single.fact::<i64>().expect("decode").data, 2);
    single.acknowledge().await.expect("ack one");

    assert!(
        reader
            .claim_batch(&consumer, 10, &[])
            .await
            .expect("claim")
            .is_none()
    );
    assert!(reader.claim(&consumer).await.expect("claim").is_none());
}

#[tokio::test]
async fn an_incomplete_fact_contract_is_refused_before_writing() {
    let pool = crate::setup_test_pool().await;
    let pool = pool.as_ref().clone();
    let outbox = DurableOutbox::new(pool.clone(), unique_instance());
    let actor = Actor::user(UserId::new("contract_user"));
    let event = AnalyticsEventBuilder::heartbeat();
    let consumer = consumer_name();

    for (name, kind, version) in [
        ("", "k", 1),
        (consumer.as_str(), " ", 1),
        (consumer.as_str(), "k", 0),
    ] {
        let mut tx = pool.begin().await.expect("begin");
        let fact = ReportingFact {
            consumer: name.to_owned(),
            kind: kind.to_owned(),
            version,
            data: (),
        };
        let err = outbox
            .append(&mut tx, &actor, SseEvent::Analytics(&event), &fact)
            .await
            .expect_err("contract refused");
        assert!(matches!(err, DurableEventError::InvalidContract), "{err:?}");
        tx.rollback().await.expect("rollback");
    }
    assert!(
        OutboxConsumer::new(pool)
            .claim(&consumer)
            .await
            .expect("claim")
            .is_none()
    );
}
