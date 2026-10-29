use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use systemprompt_extension::notifications::{NotificationRouter, NotificationRouterError};
use systemprompt_identifiers::{Actor, DocumentId, UserId};
use systemprompt_manifest::profile::{ConsumerProfile, NotificationsProfile};
use systemprompt_models::notifications::{
    DeliveryState, NOTIFICATIONS_CONSUMER, NotificationEvent, RoutePlan,
};
use systemprompt_notifications::consumer::MAX_FACT_ATTEMPTS;
use systemprompt_traits::{Dependencies, JobContext, RunnerService};
use tokio_util::sync::CancellationToken;

use crate::harness::{Harness, mention, profile};

const BOUND: Duration = Duration::from_secs(30);

struct StallingRouter {
    calls: AtomicUsize,
    stalls: usize,
}

#[async_trait]
impl NotificationRouter for StallingRouter {
    fn name(&self) -> &'static str {
        "test-stalling"
    }

    async fn route(
        &self,
        _event: &NotificationEvent,
    ) -> Result<RoutePlan, NotificationRouterError> {
        if self.calls.fetch_add(1, Ordering::SeqCst) < self.stalls {
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
        Ok(RoutePlan::default())
    }
}

fn settings() -> ConsumerProfile {
    ConsumerProfile {
        poll_secs: 1,
        sweep_secs: 3600,
        resolver_timeout_ms: 200,
        ..ConsumerProfile::default()
    }
}

fn retrying_profile() -> NotificationsProfile {
    NotificationsProfile {
        consumer: settings(),
        ..profile()
    }
}

async fn stalling(prefix: &str, stalls: usize) -> (Harness, Arc<StallingRouter>) {
    let router = Arc::new(StallingRouter {
        calls: AtomicUsize::new(0),
        stalls,
    });
    let seam = Arc::clone(&router);
    let h = Harness::with_seams(prefix, retrying_profile(), move |parts| {
        parts.routers.push(seam as Arc<dyn NotificationRouter>);
    })
    .await;
    (h, router)
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

async fn until(mut done: impl AsyncFnMut() -> bool) -> bool {
    let deadline = tokio::time::Instant::now() + BOUND;
    while tokio::time::Instant::now() < deadline {
        if done().await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

fn run(h: &Harness, actor: &UserId) -> (CancellationToken, tokio::task::JoinHandle<()>) {
    let consumer = Arc::new(h.consumer(settings()));
    let shutdown = CancellationToken::new();
    let token = shutdown.clone();
    let ctx = JobContext::new(Actor::user(actor.clone()), Dependencies::new());
    let runner = tokio::spawn(async move { consumer.run(ctx, token).await });
    (shutdown, runner)
}

#[tokio::test]
async fn a_fact_whose_router_times_out_is_retried_unacknowledged_and_then_delivered() {
    let (h, router) = stalling("notif_fact_retry", 1).await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let (event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    h.emit(&event).await;

    let (shutdown, runner) = run(&h, &author);
    let delivered = until(async || {
        h.rows_for(&reader)
            .await
            .first()
            .is_some_and(|r| r.delivery_state == DeliveryState::Delivered)
    })
    .await;
    shutdown.cancel();
    runner.await.expect("runner");

    assert!(delivered, "the retried fact reached its recipient");
    assert!(router.calls.load(Ordering::SeqCst) >= 2);
    let failures = process_failures(&h).await;
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].starts_with("route_timeout"), "{failures:?}");
    assert_eq!(unacknowledged(&h).await, 0);
    h.finish().await;
}

#[tokio::test]
async fn a_fact_that_keeps_failing_transiently_is_abandoned_after_the_attempt_cap() {
    let (h, router) = stalling("notif_fact_abandon", usize::MAX).await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let (event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    h.emit(&event).await;

    let (shutdown, runner) = run(&h, &author);
    let settled = until(async || unacknowledged(&h).await == 0).await;
    shutdown.cancel();
    runner.await.expect("runner");

    assert!(settled, "the abandoned fact was acknowledged");
    let attempts = usize::try_from(MAX_FACT_ATTEMPTS).expect("cap fits");
    assert_eq!(router.calls.load(Ordering::SeqCst), attempts);
    let failures = process_failures(&h).await;
    assert_eq!(failures.len(), attempts, "{failures:?}");
    assert!(
        failures
            .last()
            .is_some_and(|reason| reason.starts_with("fact_abandoned: route_timeout")),
        "{failures:?}"
    );
    assert!(h.rows_for(&reader).await.is_empty());
    h.finish().await;
}
