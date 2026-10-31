use std::sync::Arc;

use chrono::{Duration, Utc};
use systemprompt_brain::replay::ReplayService;
use systemprompt_events::services::durable::{
    DurableOutbox, OutboxConsumer, ReportingFact, SseEvent,
};
use systemprompt_extension::brain::{AnalysisWindow, BrainModelClient};
use systemprompt_identifiers::{Actor, PipelineVersion, UserId};
use systemprompt_models::brain::{BRAIN_CONSUMER, BRAIN_EVENT_VERSION};
use systemprompt_test_fixtures::unique_instance;

use crate::events::{Landed, price};
use crate::fakes::{MODEL_TOKENS_IN, MODEL_TOKENS_OUT};
use crate::harness::Harness;

fn window() -> AnalysisWindow {
    let until = Utc::now() + Duration::minutes(5);
    AnalysisWindow {
        since: until - Duration::days(1),
        until,
    }
}

fn version() -> PipelineVersion {
    PipelineVersion::try_new("fedcba9876543210").expect("version")
}

fn service(h: &Harness) -> ReplayService {
    ReplayService::new(
        Arc::clone(&h.repos),
        OutboxConsumer::new(h.pg.clone()),
        h.seams.clone(),
        h.profile.clone(),
    )
}

async fn landed_prices(h: &Harness, author: &UserId, products: &[&str]) {
    for product in products {
        h.model
            .push(serde_json::json!([price(product, "emea", "1200")]));
        h.emit(&Landed::new().event(author)).await;
    }
    h.drain().await;
}

async fn append_foreign_version(h: &Harness, author: &UserId) {
    let outbox = DurableOutbox::new(h.pg.clone(), unique_instance());
    let fact = ReportingFact {
        consumer: BRAIN_CONSUMER.to_owned(),
        kind: "document.landed".to_owned(),
        version: BRAIN_EVENT_VERSION + 41,
        data: serde_json::json!({"unknown": true}),
    };
    let mut tx = h.pg.begin().await.expect("begin");
    outbox
        .append_on(
            &mut tx,
            &Actor::user(author.clone()),
            SseEvent::FactOnly,
            &fact,
        )
        .await
        .expect("append");
    tx.commit().await.expect("commit");
}

#[tokio::test]
async fn a_dry_run_replays_each_event_counts_undecodable_ones_and_keeps_nothing() {
    let h = Harness::new("brain_replay_window").await;
    let author = h.user("author", &[]).await;
    landed_prices(&h, &author, &["replaya", "replayb"]).await;
    append_foreign_version(&h, &author).await;
    let facts_before = h.total("brain_facts").await;
    h.model
        .push(serde_json::json!([price("replaya", "emea", "1500")]));
    h.model
        .push(serde_json::json!([price("replayc", "emea", "900")]));
    let model: Arc<dyn BrainModelClient> = Arc::clone(&h.model) as _;

    let report = service(&h)
        .dry_run(window(), version(), model)
        .await
        .expect("dry run");

    assert_eq!(report.pipeline_version, version());
    assert_eq!(report.events, 2);
    assert_eq!(report.undecodable, 1);
    assert_eq!(report.failed, 0);
    assert!(!report.truncated);
    assert_eq!(
        report.tokens,
        2 * u64::from(MODEL_TOKENS_IN + MODEL_TOKENS_OUT)
    );
    assert_eq!(report.would_create, 1, "{report:?}");
    assert_eq!(report.would_revise + report.would_dispute, 1, "{report:?}");
    assert_eq!(report.unchanged, 0, "{report:?}");
    assert_eq!(report.per_kind.values().map(|c| c.create).sum::<u64>(), 1);
    assert_eq!(h.total("brain_facts").await, facts_before);
    h.finish().await;
}

#[tokio::test]
async fn a_dry_run_stops_at_its_event_cap_and_reports_truncation() {
    let h = Harness::new("brain_replay_capped").await;
    let author = h.user("author", &[]).await;
    landed_prices(&h, &author, &["capa", "capb", "capc"]).await;
    let model: Arc<dyn BrainModelClient> = Arc::clone(&h.model) as _;
    let calls_before = h.model.calls();

    let report = service(&h)
        .with_max_events(1)
        .dry_run(window(), version(), model)
        .await
        .expect("dry run");

    assert_eq!(report.events, 1);
    assert!(report.truncated);
    assert_eq!(h.model.calls(), calls_before + 1);
    h.finish().await;
}

#[tokio::test]
async fn a_window_before_any_event_replays_nothing() {
    let h = Harness::new("brain_replay_early").await;
    let author = h.user("author", &[]).await;
    landed_prices(&h, &author, &["earlya"]).await;
    let until = Utc::now() - Duration::days(2);
    let early = AnalysisWindow {
        since: until - Duration::days(1),
        until,
    };
    let model: Arc<dyn BrainModelClient> = Arc::clone(&h.model) as _;

    let report = service(&h)
        .dry_run(early, version(), model)
        .await
        .expect("dry run");

    assert_eq!(report.events, 0);
    assert_eq!(report.undecodable, 0);
    assert_eq!(report.tokens, 0);
    h.finish().await;
}
