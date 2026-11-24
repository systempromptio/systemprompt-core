//! Concurrent claimers of one consumer's pending rows: every row is handed
//! out exactly once at 1, 8 and 32 claimers, and the claim latency and
//! throughput of each run are printed as the measurement the per-aggregate
//! ordering decision reads (`ORDER BY created_at, id … FOR UPDATE SKIP
//! LOCKED`, one row per claim).

use std::collections::HashSet;
use std::time::{Duration, Instant};

use systemprompt_events::services::durable::{
    DurableOutbox, OutboxConsumer, ReportingFact, SseEvent,
};
use systemprompt_identifiers::{Actor, ConnectionId, EventOutboxId, UserId};
use systemprompt_models::AnalyticsEventBuilder;
use systemprompt_test_fixtures::unique_instance;

const ROWS: i64 = 640;
const CLAIMERS: [u32; 3] = [1, 8, 32];

async fn seed(pool: &sqlx::PgPool, consumer: &str) -> HashSet<EventOutboxId> {
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
        ids.insert(
            outbox
                .append(&mut tx, &actor, SseEvent::Analytics(&event), &fact)
                .await
                .expect("append"),
        );
    }
    tx.commit().await.expect("commit");
    ids
}

async fn drain(pool: sqlx::PgPool, consumer: String) -> (Vec<EventOutboxId>, Duration) {
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_claimers_take_every_row_once_and_report_throughput() {
    let scratch = crate::OutboxSchema::create().await;
    for claimers in CLAIMERS {
        let pool = scratch.pool_of(claimers + 1).await;
        let consumer = format!(
            "claim_{}",
            ConnectionId::generate().to_string().replace('-', "_")
        );
        let expected = seed(&pool, &consumer).await;

        let started = Instant::now();
        let runs: Vec<_> = (0..claimers)
            .map(|_| tokio::spawn(drain(pool.clone(), consumer.clone())))
            .collect();
        let mut claimed = Vec::new();
        let mut waiting = Duration::ZERO;
        for run in runs {
            let (ids, waited) = run.await.expect("claimer");
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
            "claim_throughput claimers={claimers} rows={ROWS} elapsed_ms={} \
             mean_claim_us={} rows_per_s={rate:.0}",
            elapsed.as_millis(),
            per_claim.as_micros()
        );
        pool.close().await;
    }
    scratch.drop_now().await;
}
