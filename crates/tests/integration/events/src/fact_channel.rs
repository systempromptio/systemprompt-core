//! An append wakes exactly one listener channel: a fact-only row notifies the
//! notification facts channel and never the relay's outbox channel, and a row
//! with a live frame notifies the outbox channel and never the facts channel.

use std::time::Duration;

use sqlx::postgres::PgListener;
use systemprompt_events::services::durable::{DurableOutbox, ReportingFact, SseEvent};
use systemprompt_events::{NOTIFICATION_FACTS_CHANNEL, OUTBOX_CHANNEL};
use systemprompt_identifiers::{Actor, ConnectionId, EventOutboxId, UserId};
use systemprompt_models::AnalyticsEventBuilder;
use systemprompt_test_fixtures::unique_instance;

async fn listener_on(pool: &sqlx::PgPool, channel: &str) -> PgListener {
    let mut listener = PgListener::connect_with(pool).await.expect("listener");
    listener.listen(channel).await.expect("listen");
    listener
}

async fn notified_within(listener: &mut PgListener, window: Duration, id: &str) -> bool {
    tokio::time::timeout(window, async {
        loop {
            if listener.recv().await.expect("recv").payload() == id {
                return;
            }
        }
    })
    .await
    .is_ok()
}

async fn append(
    pool: &sqlx::PgPool,
    outbox: &DurableOutbox,
    event: SseEvent<'_>,
    consumer: &str,
) -> EventOutboxId {
    let actor = Actor::user(UserId::new(format!("user_{consumer}")));
    let fact = ReportingFact {
        consumer: consumer.to_owned(),
        kind: "test.channel".to_owned(),
        version: 1,
        data: 1_i64,
    };
    let mut tx = pool.begin().await.expect("begin");
    let id = outbox
        .append(&mut tx, &actor, event, &fact)
        .await
        .expect("append");
    tx.commit().await.expect("commit");
    id
}

#[tokio::test]
async fn a_fact_only_append_wakes_the_facts_channel_and_not_the_relay() {
    let pool = crate::setup_test_pool().await;
    let pool = pool.as_ref().clone();
    let consumer = format!(
        "facts_{}",
        ConnectionId::generate().to_string().replace('-', "_")
    );
    let outbox = DurableOutbox::new(pool.clone(), unique_instance());
    let mut facts = listener_on(&pool, NOTIFICATION_FACTS_CHANNEL).await;
    let mut relay = listener_on(&pool, OUTBOX_CHANNEL).await;

    let fact_only = append(&pool, &outbox, SseEvent::FactOnly, &consumer).await;
    assert!(
        notified_within(&mut facts, Duration::from_secs(2), fact_only.as_str()).await,
        "a fact-only row notifies the facts channel"
    );
    assert!(
        !notified_within(&mut relay, Duration::from_millis(200), fact_only.as_str()).await,
        "a fact-only row never wakes the relay"
    );

    let heartbeat = AnalyticsEventBuilder::heartbeat();
    let live = append(&pool, &outbox, SseEvent::Analytics(&heartbeat), &consumer).await;
    assert!(
        notified_within(&mut relay, Duration::from_secs(2), live.as_str()).await,
        "a row with a live frame notifies the relay"
    );
    assert!(
        !notified_within(&mut facts, Duration::from_millis(200), live.as_str()).await,
        "a row with a live frame never wakes the facts consumers"
    );
}
