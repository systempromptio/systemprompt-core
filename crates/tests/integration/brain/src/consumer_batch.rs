//! The batched drain round: one commit per round, a transient failure that
//! does not end the round, and the cap on transient failures per round.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_brain::consumer::{BrainConsumer, MAX_ROUND_TRANSIENTS};
use systemprompt_identifiers::{Actor, EventOutboxId, UserId};
use systemprompt_manifest::profile::BrainConsumerProfile;
use systemprompt_traits::{Dependencies, JobContext, RunnerService};
use tokio_util::sync::CancellationToken;

use crate::events::{Landed, price};
use crate::harness::{Harness, Setup};

const PROCESSED: &str =
    "SELECT COUNT(*) FROM event_outbox WHERE id = $1 AND processed_at IS NOT NULL";
const RETRY_FAILURES: &str = "SELECT COUNT(*) FROM brain_failures WHERE stage <> $1";

/// A poll interval no test waits out: anything the consumer does inside the
/// deadline happened in a round it ran straight away.
fn slow_poll() -> Setup {
    let mut setup = Setup::default();
    setup.profile.consumer = BrainConsumerProfile {
        poll_secs: 3600,
        sweep_secs: 3600,
        ..BrainConsumerProfile::default()
    };
    setup
}

async fn run_until(
    h: &Harness,
    sql: &'static str,
    bind: &str,
    at_least: i64,
) -> Arc<BrainConsumer> {
    let consumer: Arc<BrainConsumer> = Arc::new(h.consumer());
    let shutdown = CancellationToken::new();
    let runner = {
        let consumer = Arc::clone(&consumer);
        let shutdown = shutdown.clone();
        let ctx = JobContext::new(Actor::user(UserId::generate()), Dependencies::new());
        tokio::spawn(async move { consumer.run(ctx, shutdown).await })
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while tokio::time::Instant::now() < deadline && h.count(sql, bind).await < at_least {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    shutdown.cancel();
    runner.await.expect("consumer stops");
    consumer
}

async fn attempts(h: &Harness, id: &EventOutboxId) -> i64 {
    h.count(
        "SELECT COUNT(*) FROM brain_failures WHERE outbox_id = $1",
        id.as_str(),
    )
    .await
}

#[tokio::test]
async fn a_transient_failure_does_not_end_the_round() {
    let h = Harness::with("brain_batch_transient", slow_poll()).await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.model.push_unavailable();
    h.model
        .push(serde_json::json!([price("globex", "emea", "900")]));
    let first = h.emit(&Landed::new().event(&author)).await;
    let flaky = h.emit(&Landed::new().event(&author)).await;
    let last = h.emit(&Landed::new().event(&author)).await;

    let consumer = run_until(&h, PROCESSED, last.as_str(), 1).await;

    assert_eq!(h.count(PROCESSED, first.as_str()).await, 1);
    assert_eq!(
        h.count(PROCESSED, last.as_str()).await,
        1,
        "the event after the transient one commits in the same round"
    );
    assert_eq!(
        h.count(PROCESSED, flaky.as_str()).await,
        0,
        "held for retry"
    );
    assert_eq!(attempts(&h, &flaky).await, 1);
    let backlog = consumer.backlog();
    assert_eq!(backlog.processed, 2);
    assert_eq!(backlog.retried, 1);
    assert!(backlog.last_round.is_some());
    assert_eq!(h.total("brain_facts").await, 2);
    let pending = consumer.pending().await.expect("pending");
    assert_eq!(pending.pending, 1, "the held event is still pending");
    h.finish().await;
}

#[tokio::test]
async fn too_many_transient_failures_cut_the_round_short() {
    let h = Harness::with("brain_batch_cap", slow_poll()).await;
    let author = h.user("author", &[]).await;
    let cap = MAX_ROUND_TRANSIENTS + 1;
    let mut ids = Vec::new();
    for _ in 0..=cap {
        h.model.push_unavailable();
        ids.push(h.emit(&Landed::new().event(&author)).await);
    }
    let reached = i64::try_from(cap).expect("cap");

    let consumer = run_until(&h, RETRY_FAILURES, "decode", reached).await;

    assert_eq!(h.model.calls(), cap, "the round stops after the cap");
    assert_eq!(h.count(RETRY_FAILURES, "decode").await, reached);
    let untouched = ids.last().expect("an event past the cap");
    assert_eq!(attempts(&h, untouched).await, 0);
    assert_eq!(h.count(PROCESSED, untouched.as_str()).await, 0);
    assert_eq!(consumer.backlog().retried, u64::try_from(cap).expect("cap"));
    let pending = consumer.pending().await.expect("pending");
    assert_eq!(pending.pending, i64::try_from(cap + 1).expect("pending"));
    h.finish().await;
}

#[tokio::test]
async fn a_round_with_a_bad_event_commits_the_good_ones() {
    let h = Harness::with("brain_batch_mixed", slow_poll()).await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.model
        .push(serde_json::json!([price("globex", "emea", "900")]));
    let first = h.emit(&Landed::new().event(&author)).await;
    let bad = h.emit(&Landed::new().event(&author)).await;
    sqlx::query("UPDATE event_outbox SET fact = jsonb_set(fact, '{version}', '999') WHERE id = $1")
        .bind(bad.as_str())
        .execute(&h.pg)
        .await
        .expect("rewrite fact");
    let last = h.emit(&Landed::new().event(&author)).await;

    let consumer = run_until(&h, PROCESSED, last.as_str(), 1).await;

    for id in [&first, &bad, &last] {
        assert_eq!(h.count(PROCESSED, id.as_str()).await, 1);
    }
    let backlog = consumer.backlog();
    assert_eq!(backlog.processed, 2);
    assert_eq!(backlog.failed, 1);
    assert_eq!(backlog.retried, 0);
    assert_eq!(h.total("brain_facts").await, 2);
    assert_eq!(consumer.pending().await.expect("pending").pending, 0);
    h.finish().await;
}
