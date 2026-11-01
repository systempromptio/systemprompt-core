use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use systemprompt_brain::consumer::{BacklogSnapshot, BrainConsumer, MAX_EVENT_ATTEMPTS};
use systemprompt_identifiers::{Actor, EventOutboxId, UserId};
use systemprompt_manifest::profile::BrainConsumerProfile;
use systemprompt_models::brain::{BrainEvent, BrainEventKind};
use systemprompt_traits::{Dependencies, JobContext, RunnerService};
use tokio_util::sync::CancellationToken;

use crate::events::{Landed, price};
use crate::harness::{Harness, Setup};

const PROCESSED: &str =
    "SELECT COUNT(*) FROM event_outbox WHERE id = $1 AND processed_at IS NOT NULL";

fn fast_poll(mut setup: Setup) -> Setup {
    setup.profile.consumer = BrainConsumerProfile {
        poll_secs: 1,
        sweep_secs: 3600,
        ..BrainConsumerProfile::default()
    };
    setup
}

async fn run_until(h: &Harness, sql: &'static str, bind: &str, at_least: i64) -> BacklogSnapshot {
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
    consumer.backlog()
}

async fn failures(h: &Harness, id: &EventOutboxId) -> Vec<(String, i32)> {
    sqlx::query_as::<_, (String, i32)>(
        "SELECT stage, attempt FROM brain_failures WHERE outbox_id = $1 \
         ORDER BY attempt, created_at",
    )
    .bind(id.as_str())
    .fetch_all(&h.pg)
    .await
    .expect("failures")
}

async fn outcome(h: &Harness, id: &EventOutboxId) -> Option<String> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT outcome FROM brain_event_results WHERE outbox_id = $1",
    )
    .bind(id.as_str())
    .fetch_optional(&h.pg)
    .await
    .expect("event result")
    .flatten()
}

async fn rewrite_fact(h: &Harness, id: &EventOutboxId, sql: &'static str) {
    sqlx::query(sql)
        .bind(id.as_str())
        .execute(&h.pg)
        .await
        .expect("rewrite fact");
}

#[tokio::test]
async fn a_healthy_event_is_processed_and_acknowledged() {
    let h = Harness::with("brain_drain_ok", fast_poll(Setup::default())).await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    let id = h.emit(&Landed::new().event(&author)).await;

    let backlog = run_until(&h, PROCESSED, id.as_str(), 1).await;

    assert_eq!(h.count(PROCESSED, id.as_str()).await, 1);
    assert_eq!(backlog.processed, 1);
    assert_eq!(backlog.retried, 0);
    assert_eq!(backlog.failed, 0);
    assert_eq!(backlog.abandoned, 0);
    assert!(failures(&h, &id).await.is_empty());
    assert_eq!(h.total("brain_facts").await, 1);
    h.finish().await;
}

#[tokio::test]
async fn a_transient_failure_is_retried_until_it_is_abandoned() {
    let mut setup = fast_poll(Setup::default());
    setup.profile.extraction.daily_token_budget = 10;
    let h = Harness::with("brain_drain_abandon", setup).await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.emit(&Landed::new().event(&author)).await;
    h.drain().await;
    let spent = h.emit(&Landed::new().event(&author)).await;

    let backlog = run_until(&h, PROCESSED, spent.as_str(), 1).await;

    let attempts = failures(&h, &spent).await;
    let max = i32::try_from(MAX_EVENT_ATTEMPTS).expect("attempts");
    let mut expected: Vec<(String, i32)> = (1..max).map(|n| ("budget".to_owned(), n)).collect();
    expected.push(("abandoned".to_owned(), max));
    assert_eq!(attempts, expected);
    assert_eq!(backlog.retried, u64::try_from(max - 1).expect("retries"));
    assert_eq!(backlog.abandoned, 1);
    assert_eq!(backlog.processed, 0);
    assert_eq!(outcome(&h, &spent).await.as_deref(), Some("abandoned"));
    assert_eq!(h.count(PROCESSED, spent.as_str()).await, 1);
    assert_eq!(
        h.model.calls(),
        1,
        "the model is never called past the budget"
    );
    let reason: String = sqlx::query_scalar(
        "SELECT reason FROM brain_failures WHERE outbox_id = $1 AND stage = 'abandoned'",
    )
    .bind(spent.as_str())
    .fetch_one(&h.pg)
    .await
    .expect("abandon reason");
    assert!(reason.starts_with("event_abandoned: "), "{reason}");
    h.finish().await;
}

#[tokio::test]
async fn an_event_one_attempt_short_of_the_limit_is_abandoned_on_its_next_failure() {
    let mut setup = fast_poll(Setup::default());
    setup.profile.extraction.daily_token_budget = 10;
    let h = Harness::with("brain_drain_seeded", setup).await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.emit(&Landed::new().event(&author)).await;
    h.drain().await;
    let spent = h.emit(&Landed::new().event(&author)).await;
    for attempt in 1..MAX_EVENT_ATTEMPTS {
        sqlx::query(
            "INSERT INTO brain_failures (id, outbox_id, stage, reason, attempt) \
             VALUES (gen_random_uuid()::text, $1, 'budget', 'seeded', $2)",
        )
        .bind(spent.as_str())
        .bind(i32::try_from(attempt).expect("attempt"))
        .execute(&h.pg)
        .await
        .expect("seed failure");
    }

    let backlog = run_until(&h, PROCESSED, spent.as_str(), 1).await;

    assert_eq!(backlog.retried, 0, "the fifth failure is not retried");
    assert_eq!(backlog.abandoned, 1);
    assert_eq!(
        failures(&h, &spent).await.last().cloned(),
        Some((
            "abandoned".to_owned(),
            i32::try_from(MAX_EVENT_ATTEMPTS).expect("attempts")
        ))
    );
    assert_eq!(outcome(&h, &spent).await.as_deref(), Some("abandoned"));
    h.finish().await;
}

#[tokio::test]
async fn an_event_of_an_unread_version_fails_at_decode_and_is_acknowledged() {
    let h = Harness::with("brain_drain_version", fast_poll(Setup::default())).await;
    let author = h.user("author", &[]).await;
    let id = h.emit(&Landed::new().event(&author)).await;
    rewrite_fact(
        &h,
        &id,
        "UPDATE event_outbox SET fact = jsonb_set(fact, '{version}', '999') WHERE id = $1",
    )
    .await;

    let backlog = run_until(&h, PROCESSED, id.as_str(), 1).await;

    assert_eq!(failures(&h, &id).await, vec![("decode".to_owned(), 1)]);
    let reason: String =
        sqlx::query_scalar("SELECT reason FROM brain_failures WHERE outbox_id = $1")
            .bind(id.as_str())
            .fetch_one(&h.pg)
            .await
            .expect("reason");
    assert_eq!(reason, "unsupported event version 999");
    assert_eq!(backlog.failed, 1);
    assert_eq!(backlog.retried, 0);
    assert_eq!(
        outcome(&h, &id).await,
        None,
        "a decode failure claims no result"
    );
    assert_eq!(h.model.calls(), 0);
    h.finish().await;
}

#[tokio::test]
async fn an_event_whose_data_does_not_decode_fails_at_decode() {
    let h = Harness::with("brain_drain_json", fast_poll(Setup::default())).await;
    let author = h.user("author", &[]).await;
    let id = h.emit(&Landed::new().event(&author)).await;
    rewrite_fact(
        &h,
        &id,
        "UPDATE event_outbox SET fact = jsonb_set(fact, '{data}', '\"garbage\"') WHERE id = $1",
    )
    .await;

    let backlog = run_until(&h, PROCESSED, id.as_str(), 1).await;

    assert_eq!(failures(&h, &id).await, vec![("decode".to_owned(), 1)]);
    assert_eq!(backlog.failed, 1);
    assert_eq!(backlog.processed, 0);
    h.finish().await;
}

#[tokio::test]
async fn an_event_kind_the_pipeline_does_not_read_fails_permanently() {
    let h = Harness::with("brain_drain_unknown", fast_poll(Setup::default())).await;
    let author = h.user("author", &[]).await;
    let kind = BrainEventKind::new("acme.custom.happened").expect("kind");
    let id = h
        .emit(&BrainEvent::new(kind, author.clone(), Utc::now()))
        .await;

    let backlog = run_until(&h, PROCESSED, id.as_str(), 1).await;

    assert_eq!(failures(&h, &id).await, vec![("process".to_owned(), 1)]);
    assert_eq!(outcome(&h, &id).await.as_deref(), Some("failed"));
    assert_eq!(backlog.failed, 1);
    assert_eq!(backlog.retried, 0);
    assert_eq!(backlog.abandoned, 0);
    assert_eq!(h.count(PROCESSED, id.as_str()).await, 1);
    h.finish().await;
}
