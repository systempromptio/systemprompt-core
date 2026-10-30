use std::sync::Arc;
use std::time::Duration;

use systemprompt_identifiers::{Actor, AiRequestId, UserId};
use systemprompt_manifest::profile::BrainConsumerProfile;
use systemprompt_traits::{Dependencies, JobContext, RunnerService};
use tokio_util::sync::CancellationToken;

use crate::events::{Landed, price};
use crate::fakes::{MODEL_TOKENS_IN, MODEL_TOKENS_OUT};
use crate::harness::{Harness, Setup};

const EXTRACTIONS: &str = "SELECT COUNT(*) FROM brain_extractions WHERE outbox_id = $1";

#[tokio::test]
async fn an_event_drained_twice_is_extracted_and_billed_once() {
    let h = Harness::new("brain_extract_once").await;
    let author = h.user("author", &[]).await;
    let request = AiRequestId::generate();
    h.model.with_request_id(request.clone());
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    let event = Landed::new().event(&author);
    let id = h.emit(&event).await;

    h.drain().await;
    h.reprocess(&id, event).await;

    assert_eq!(h.count(EXTRACTIONS, id.as_str()).await, 1);
    let (input, output, ai_request): (i32, i32, Option<String>) = sqlx::query_as(
        "SELECT input_tokens, output_tokens, ai_request_id FROM brain_extractions \
         WHERE outbox_id = $1",
    )
    .bind(id.as_str())
    .fetch_one(&h.pg)
    .await
    .expect("extraction row");
    assert_eq!(input, i32::try_from(MODEL_TOKENS_IN).expect("tokens"));
    assert_eq!(output, i32::try_from(MODEL_TOKENS_OUT).expect("tokens"));
    assert_eq!(ai_request.as_deref(), Some(request.as_str()));

    let other = h.emit(&Landed::new().event(&author)).await;
    h.drain().await;
    assert_eq!(
        h.count(EXTRACTIONS, other.as_str()).await,
        1,
        "negative control: a distinct event is extracted on its own"
    );
    let missing: Option<String> =
        sqlx::query_scalar("SELECT ai_request_id FROM brain_extractions WHERE outbox_id = $1")
            .bind(other.as_str())
            .fetch_one(&h.pg)
            .await
            .expect("second extraction");
    assert_eq!(missing.as_deref(), Some(request.as_str()));
    h.finish().await;
}

#[tokio::test]
async fn an_extraction_without_a_gateway_request_records_a_null_request_id() {
    let h = Harness::new("brain_extract_no_request").await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    let id = h.emit(&Landed::new().event(&author)).await;
    h.drain().await;

    let ai_request: Option<String> =
        sqlx::query_scalar("SELECT ai_request_id FROM brain_extractions WHERE outbox_id = $1")
            .bind(id.as_str())
            .fetch_one(&h.pg)
            .await
            .expect("extraction row");
    assert_eq!(ai_request, None);
    h.finish().await;
}

async fn run_until_failure(h: &Harness, outbox: &str) -> i64 {
    let consumer = Arc::new(h.consumer());
    let shutdown = CancellationToken::new();
    let runner = {
        let consumer = Arc::clone(&consumer);
        let shutdown = shutdown.clone();
        let ctx = JobContext::new(Actor::user(UserId::generate()), Dependencies::new());
        tokio::spawn(async move { consumer.run(ctx, shutdown).await })
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut failures = 0;
    while tokio::time::Instant::now() < deadline {
        failures = h
            .count(
                "SELECT COUNT(*) FROM brain_failures WHERE outbox_id = $1 AND stage = 'budget'",
                outbox,
            )
            .await;
        if failures > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    shutdown.cancel();
    runner.await.expect("consumer stops");
    failures
}

#[tokio::test]
async fn a_spent_daily_budget_fails_the_event_transiently_at_the_budget_stage() {
    let mut setup = Setup::default();
    setup.profile.extraction.daily_token_budget = 10;
    setup.profile.consumer = BrainConsumerProfile {
        poll_secs: 3600,
        sweep_secs: 3600,
        ..BrainConsumerProfile::default()
    };
    let h = Harness::with("brain_extract_budget", setup).await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.emit(&Landed::new().event(&author)).await;
    h.drain().await;
    assert_eq!(
        h.total("brain_facts").await,
        1,
        "negative control: the first event fit the budget"
    );

    let spent = h.emit(&Landed::new().event(&author)).await;
    let failures = run_until_failure(&h, spent.as_str()).await;

    assert_eq!(failures, 1, "one transient failure at the budget stage");
    assert_eq!(
        h.model.calls(),
        1,
        "the model was not called past the budget"
    );
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM brain_event_results WHERE outbox_id = $1",
            spent.as_str()
        )
        .await,
        0,
        "the event's claim rolled back for a retry"
    );
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM event_outbox WHERE id = $1 AND processed_at IS NULL",
            spent.as_str()
        )
        .await,
        1,
        "the event stays pending"
    );
    h.finish().await;
}
