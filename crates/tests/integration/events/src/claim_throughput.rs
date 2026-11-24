//! Concurrent claimers of one consumer's pending rows: every row is handed
//! out exactly once at 1, 8 and 32 claimers, and the claim latency and
//! throughput of each run are printed as the measurement the per-aggregate
//! ordering decision reads. Each run is taken twice on the same seed shape:
//! `shape=before` claims with the predicate the outbox used before
//! per-aggregate order (`ORDER BY created_at, id … FOR UPDATE SKIP LOCKED`,
//! held here as SQL text), `shape=after` claims through
//! [`OutboxConsumer::claim`]. Unkeyed and keyed seeds are both measured.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use systemprompt_events::services::durable::{
    DurableOutbox, OutboxConsumer, ReportingFact, SseEvent,
};
use systemprompt_identifiers::{Actor, ConnectionId, EventOutboxId, OutboxAggregateKey, UserId};
use systemprompt_models::AnalyticsEventBuilder;
use systemprompt_test_fixtures::unique_instance;

const ROWS: i64 = 640;
const AGGREGATES: i64 = 64;
const CLAIMERS: [u32; 3] = [1, 8, 32];

const BEFORE_CLAIM: &str = "SELECT id FROM event_outbox \
     WHERE consumer = $1 AND processed_at IS NULL \
     ORDER BY created_at, id LIMIT 1 FOR UPDATE SKIP LOCKED";
const ACKNOWLEDGE: &str = "UPDATE event_outbox SET processed_at = now() WHERE id = $1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Before,
    After,
}

impl Shape {
    const fn label(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }
}

async fn seed(pool: &sqlx::PgPool, consumer: &str, keyed: bool) -> HashSet<EventOutboxId> {
    let outbox = DurableOutbox::new(pool.clone(), unique_instance());
    let actor = Actor::user(UserId::new(format!("user_{consumer}")));
    let event = AnalyticsEventBuilder::heartbeat();
    let mut ids = HashSet::new();
    let mut tx = pool.begin().await.expect("begin");
    for value in 0..ROWS {
        let fact = ReportingFact {
            consumer: consumer.to_owned(),
            kind: "test.claim_throughput".to_owned(),
            version: 1,
            data: value,
        };
        let id = if keyed {
            let aggregate = OutboxAggregateKey::new(format!("agg_{}", value % AGGREGATES));
            outbox
                .append_in_order_on(
                    tx.as_mut(),
                    &actor,
                    SseEvent::Analytics(&event),
                    &fact,
                    &aggregate,
                )
                .await
        } else {
            outbox
                .append(&mut tx, &actor, SseEvent::Analytics(&event), &fact)
                .await
        };
        ids.insert(id.expect("append"));
    }
    tx.commit().await.expect("commit");
    ids
}

async fn drain_after(pool: sqlx::PgPool, consumer: String) -> (Vec<EventOutboxId>, Duration) {
    let reader = OutboxConsumer::new(pool);
    let mut claimed = Vec::new();
    let mut waiting = Duration::ZERO;
    loop {
        let started = Instant::now();
        let Some(delivery) = reader.claim(&consumer).await.expect("claim") else {
            return (claimed, waiting);
        };
        waiting += started.elapsed();
        claimed.push(delivery.id());
        delivery.acknowledge().await.expect("acknowledge");
    }
}

async fn drain_before(pool: sqlx::PgPool, consumer: String) -> (Vec<EventOutboxId>, Duration) {
    let mut claimed = Vec::new();
    let mut waiting = Duration::ZERO;
    loop {
        let started = Instant::now();
        let mut tx = pool.begin().await.expect("begin");
        let row: Option<(String,)> = sqlx::query_as(BEFORE_CLAIM)
            .bind(&consumer)
            .fetch_optional(&mut *tx)
            .await
            .expect("claim");
        let Some((id,)) = row else {
            tx.rollback().await.expect("rollback");
            return (claimed, waiting);
        };
        waiting += started.elapsed();
        sqlx::query(ACKNOWLEDGE)
            .bind(&id)
            .execute(&mut *tx)
            .await
            .expect("acknowledge");
        tx.commit().await.expect("commit");
        claimed.push(EventOutboxId::new(id));
    }
}

async fn run(scratch: &crate::OutboxSchema, shape: Shape, keyed: bool, claimers: u32) {
    let pool = scratch.pool_of(claimers + 1).await;
    let consumer = format!(
        "claim_{}",
        ConnectionId::generate().to_string().replace('-', "_")
    );
    let expected = seed(&pool, &consumer, keyed).await;

    let started = Instant::now();
    let runs: Vec<_> = (0..claimers)
        .map(|_| match shape {
            Shape::Before => tokio::spawn(drain_before(pool.clone(), consumer.clone())),
            Shape::After => tokio::spawn(drain_after(pool.clone(), consumer.clone())),
        })
        .collect();
    let mut claimed = Vec::new();
    let mut waiting = Duration::ZERO;
    for handle in runs {
        let (ids, waited) = handle.await.expect("claimer");
        claimed.extend(ids);
        waiting += waited;
    }
    let elapsed = started.elapsed();

    let distinct: HashSet<EventOutboxId> = claimed.iter().cloned().collect();
    assert_eq!(claimed.len(), distinct.len(), "a row was claimed twice");
    assert_eq!(distinct, expected, "every pending row is claimed");
    let count = u32::try_from(claimed.len()).expect("count");
    let per_claim = waiting / count;
    let rate = f64::from(count) / elapsed.as_secs_f64();
    println!(
        "claim_throughput shape={} keyed={keyed} claimers={claimers} rows={ROWS} \
         elapsed_ms={} mean_claim_us={} rows_per_s={rate:.0}",
        shape.label(),
        elapsed.as_millis(),
        per_claim.as_micros()
    );
    pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_claimers_take_every_row_once_and_report_throughput() {
    let scratch = crate::OutboxSchema::create().await;
    for keyed in [false, true] {
        for claimers in CLAIMERS {
            for shape in [Shape::Before, Shape::After] {
                run(&scratch, shape, keyed, claimers).await;
            }
        }
    }
    scratch.drop_now().await;
}
