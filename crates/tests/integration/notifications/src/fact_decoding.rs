use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use systemprompt_events::services::durable::{DurableOutbox, ReportingFact, SseEvent};
use systemprompt_identifiers::{Actor, DocumentId, UserId};
use systemprompt_manifest::profile::ConsumerProfile;
use systemprompt_models::notifications::{
    DeliveryState, NOTIFICATION_EVENT_VERSION, NOTIFICATIONS_CONSUMER, NotificationKind,
};
use systemprompt_test_fixtures::unique_instance;
use systemprompt_traits::{Dependencies, JobContext, RunnerService};
use tokio_util::sync::CancellationToken;

use crate::harness::{Harness, mention};

const BOUND: Duration = Duration::from_secs(30);

fn settings() -> ConsumerProfile {
    ConsumerProfile {
        poll_secs: 1,
        sweep_secs: 3600,
        ..ConsumerProfile::default()
    }
}

async fn append(h: &Harness, actor: &UserId, version: u32, data: &serde_json::Value) {
    let outbox = DurableOutbox::new(h.pool.write_pool().as_ref().clone(), unique_instance());
    let fact = ReportingFact {
        consumer: NOTIFICATIONS_CONSUMER.to_owned(),
        kind: NotificationKind::MENTIONED.to_owned(),
        version,
        data,
    };
    let mut tx = h.pool.write_pool().begin().await.expect("begin");
    outbox
        .append_on(
            &mut tx,
            &Actor::user(actor.clone()),
            SseEvent::FactOnly,
            &fact,
        )
        .await
        .expect("append");
    tx.commit().await.expect("commit");
}

async fn decode_failures(h: &Harness) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT reason FROM notification_delivery_failures WHERE stage = 'decode' ORDER BY id",
    )
    .fetch_all(h.pool.write_pool().as_ref())
    .await
    .expect("decode failures")
}

async fn process_failures(h: &Harness) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT reason FROM notification_delivery_failures WHERE stage = 'process' ORDER BY id",
    )
    .fetch_all(h.pool.write_pool().as_ref())
    .await
    .expect("process failures")
}

async fn unacknowledged(h: &Harness) -> i64 {
    h.count(
        "SELECT COUNT(*) FROM event_outbox WHERE consumer = $1 AND processed_at IS NULL",
        NOTIFICATIONS_CONSUMER,
    )
    .await
}

async fn drain_until(h: &Harness, actor: &UserId, mut done: impl AsyncFnMut() -> bool) -> bool {
    let consumer = Arc::new(h.consumer(settings()));
    let shutdown = CancellationToken::new();
    let token = shutdown.clone();
    let ctx = JobContext::new(Actor::user(actor.clone()), Dependencies::new());
    let runner = tokio::spawn(async move { consumer.run(ctx, token).await });
    let deadline = tokio::time::Instant::now() + BOUND;
    let mut reached = false;
    while tokio::time::Instant::now() < deadline {
        if done().await {
            reached = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    shutdown.cancel();
    runner.await.expect("runner");
    reached
}

#[tokio::test]
async fn undecodable_facts_are_recorded_as_decode_failures_and_acknowledged() {
    let h = Harness::new("notif_fact_decode").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let (event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());

    append(
        &h,
        &author,
        NOTIFICATION_EVENT_VERSION + 98,
        &serde_json::to_value(&event).expect("event json"),
    )
    .await;
    append(
        &h,
        &author,
        NOTIFICATION_EVENT_VERSION,
        &serde_json::json!({"kind": 7}),
    )
    .await;
    sqlx::query(
        "INSERT INTO event_outbox (id, channel, user_id, payload, actor_kind, actor_id, \
         origin_instance_id, consumer, fact) SELECT gen_random_uuid()::text, channel, user_id, \
         payload, actor_kind, actor_id, origin_instance_id, consumer, '{\"nope\": true}'::jsonb \
         FROM event_outbox LIMIT 1",
    )
    .execute(h.pool.write_pool().as_ref())
    .await
    .expect("envelope without a version");
    h.emit(&event).await;

    let delivered = drain_until(&h, &author, async || {
        h.rows_for(&reader)
            .await
            .first()
            .is_some_and(|row| row.delivery_state == DeliveryState::Delivered)
            && unacknowledged(&h).await == 0
    })
    .await;

    assert!(
        delivered,
        "the valid fact behind the broken ones was delivered"
    );
    let failures = decode_failures(&h).await;
    assert_eq!(failures.len(), 3, "{failures:?}");
    let expected = format!(
        "unsupported fact version {}",
        NOTIFICATION_EVENT_VERSION + 98
    );
    assert!(failures.contains(&expected), "{failures:?}");
    assert!(process_failures(&h).await.is_empty());
    assert_eq!(h.rows_for(&reader).await.len(), 1);
    h.finish().await;
}

#[tokio::test]
async fn a_subject_no_visibility_provider_names_is_withheld_and_recorded() {
    let h = Harness::new("notif_no_provider").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let event = systemprompt_models::notifications::NotificationEvent::new(
        NotificationKind::new(NotificationKind::MENTIONED).expect("kind"),
        author.clone(),
        Utc::now(),
    )
    .with_subjects([systemprompt_models::notifications::SubjectRef::new(
        "widget", "w-1",
    )])
    .with_direct_recipient(reader.clone(), None)
    .with_body("about a widget");
    append(
        &h,
        &author,
        NOTIFICATION_EVENT_VERSION,
        &serde_json::to_value(&event).expect("event json"),
    )
    .await;

    let settled = drain_until(&h, &author, async || unacknowledged(&h).await == 0).await;
    assert!(settled, "the fact was never acknowledged");

    let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT stage, reason, recipient_user_id FROM notification_delivery_failures ORDER BY id",
    )
    .fetch_all(h.pool.write_pool().as_ref())
    .await
    .expect("failures");
    let delivered = h
        .count(
            "SELECT COUNT(*) FROM notifications WHERE recipient_user_id = $1",
            reader.as_str(),
        )
        .await;
    assert_eq!(delivered, 0);
    assert_eq!(
        rows,
        vec![(
            "visibility".to_owned(),
            "no_visibility_provider".to_owned(),
            Some(reader.as_str().to_owned()),
        )]
    );
}
