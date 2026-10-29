use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use metrics_util::debugging::DebuggingRecorder;
use systemprompt_identifiers::{Actor, DocumentId, UserId};
use systemprompt_manifest::profile::ConsumerProfile;
use systemprompt_models::notifications::DeliveryState;
use systemprompt_notifications::{ConsumerState, NotificationsConsumer};
use systemprompt_traits::{Dependencies, JobContext, RunnerService};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::harness::{Harness, mention};

const BOUND: Duration = Duration::from_secs(10);

fn idle_settings() -> ConsumerProfile {
    ConsumerProfile {
        poll_secs: 3600,
        sweep_secs: 3600,
        ..ConsumerProfile::default()
    }
}

fn start(
    consumer: &Arc<NotificationsConsumer>,
    actor: &UserId,
    shutdown: &CancellationToken,
) -> JoinHandle<()> {
    let consumer = Arc::clone(consumer);
    let shutdown = shutdown.clone();
    let ctx = JobContext::new(Actor::user(actor.clone()), Dependencies::new());
    tokio::spawn(async move { consumer.run(ctx, shutdown).await })
}

async fn delivered_row(h: &Harness, reader: &UserId) -> bool {
    let deadline = tokio::time::Instant::now() + BOUND;
    while tokio::time::Instant::now() < deadline {
        let rows = h.rows_for(reader).await;
        if rows
            .first()
            .is_some_and(|r| r.delivery_state == DeliveryState::Delivered)
        {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

#[tokio::test]
async fn the_consumer_wakes_on_the_facts_channel_long_before_its_poll() {
    let h = Harness::new("notif_consumer_wake").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let consumer = Arc::new(h.consumer(idle_settings()));
    let shutdown = CancellationToken::new();
    let runner = start(&consumer, &author, &shutdown);

    let deadline = tokio::time::Instant::now() + BOUND;
    loop {
        let state = consumer.backlog().await.expect("backlog").state;
        if state == ConsumerState::Listening {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the consumer never listened: {state:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let (event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    h.emit(&event).await;
    assert!(
        delivered_row(&h, &reader).await,
        "a fact is processed within seconds while the poll interval is an hour"
    );
    let backlog = consumer.backlog().await.expect("backlog");
    assert_eq!(backlog.pending, 0);
    assert_eq!(backlog.oldest_seconds, None);

    shutdown.cancel();
    runner.await.expect("runner");
    h.finish().await;
}

const SERIES: [&str; 6] = [
    "notifications_facts_total",
    "notifications_recipients_total",
    "notifications_deliveries_total",
    "notifications_batch_seconds",
    "notifications_backlog_pending",
    "notifications_backlog_oldest_seconds",
];

#[test]
fn draining_one_fact_records_every_notifications_series() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(async {
                let h = Harness::new("notif_consumer_metrics").await;
                let author = h.user("author", &[]).await;
                let reader = h.user("reader", &[]).await;
                let (event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
                h.emit(&event).await;
                let consumer = Arc::new(h.consumer(idle_settings()));
                let shutdown = CancellationToken::new();
                let runner = start(&consumer, &author, &shutdown);
                assert!(delivered_row(&h, &reader).await, "the fact was drained");
                consumer.backlog().await.expect("backlog");
                shutdown.cancel();
                runner.await.expect("runner");
                h.finish().await;
            });
    });
    let recorded: HashSet<String> = snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .map(|(composite, _, _, _)| composite.key().name().to_owned())
        .collect();
    for name in SERIES {
        assert!(recorded.contains(name), "{name} missing from {recorded:?}");
    }
}
