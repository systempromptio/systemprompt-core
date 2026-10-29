use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use systemprompt_extension::notifications::{ChannelError, NotificationChannel};
use systemprompt_identifiers::{DocumentId, NotificationId, UserId};
use systemprompt_manifest::profile::NotificationsProfile;
use systemprompt_models::notifications::{Attention, Notification};
use systemprompt_notifications::pipeline::{DELIVERY_ABANDONED, MAX_DELIVERY_ATTEMPTS};
use systemprompt_notifications::repository::{NotificationRepository, SWEEP_LOCK_KEY};

use crate::harness::{Harness, delivery_state, profile, share};

#[derive(Clone, Copy)]
enum Step {
    Deliver,
    Unavailable,
    Undeliverable,
    Hang,
    Panic,
}

struct Scripted {
    script: Mutex<VecDeque<Step>>,
    then: Step,
    calls: AtomicUsize,
}

impl Scripted {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl NotificationChannel for Scripted {
    fn surface(&self) -> &'static str {
        "scripted"
    }

    fn delivers(&self, _attention: Attention) -> bool {
        true
    }

    async fn deliver(
        &self,
        _notification: &Notification,
        _unread: i64,
    ) -> Result<(), ChannelError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let step = self
            .script
            .lock()
            .expect("lock")
            .pop_front()
            .unwrap_or(self.then);
        match step {
            Step::Deliver => Ok(()),
            Step::Unavailable => Err(ChannelError::Unavailable {
                surface: "scripted",
            }),
            Step::Undeliverable => Err(ChannelError::Encode(
                serde_json::from_str::<u8>("not a number").expect_err("invalid json"),
            )),
            Step::Hang => {
                tokio::time::sleep(Duration::from_secs(5)).await;
                Ok(())
            },
            Step::Panic => panic!("scripted channel panicked"),
        }
    }
}

fn quick_channels() -> NotificationsProfile {
    let mut profile = profile();
    profile.consumer.channel_timeout_ms = 100;
    profile
}

async fn scripted(prefix: &str, script: &[Step], then: Step) -> (Harness, Arc<Scripted>) {
    let channel = Arc::new(Scripted {
        script: Mutex::new(script.iter().copied().collect()),
        then,
        calls: AtomicUsize::new(0),
    });
    let seam = Arc::clone(&channel);
    let h = Harness::with_seams(prefix, quick_channels(), move |parts| {
        parts.channels.push(seam as Arc<dyn NotificationChannel>);
    })
    .await;
    (h, channel)
}

async fn one_pending(h: &Harness) -> (NotificationId, UserId) {
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    h.emit(&share(&author, &reader, &DocumentId::generate()))
        .await;
    let processed = h.process_pending().await;
    assert_eq!(processed.pending.len(), 1);
    (processed.pending[0].clone(), reader)
}

async fn stage_count(h: &Harness, stage: &'static str) -> i64 {
    h.count(
        "SELECT COUNT(*) FROM notification_delivery_failures WHERE stage = $1",
        stage,
    )
    .await
}

#[tokio::test]
async fn a_row_re_pended_after_its_pass_read_it_is_not_marked_delivered() {
    let h = Harness::new("notif_delivery_seq").await;
    let (id, recipient) = one_pending(&h).await;
    let (_, read_at) = delivery_state(&h, &id).await;

    h.repos
        .notifications
        .escalate(&recipient, &id, None)
        .await
        .expect("escalate")
        .expect("escalated");
    let (state, seq) = delivery_state(&h, &id).await;
    assert_eq!((state.as_str(), seq), ("pending", read_at + 1));

    let mut conn = h.pool.write_pool().acquire().await.expect("connection");
    let marked = NotificationRepository::mark_delivered(&mut conn, &[(id.clone(), read_at)])
        .await
        .expect("mark");
    assert_eq!(marked, 0, "a stale sequence never closes the row");
    assert_eq!(delivery_state(&h, &id).await.0, "pending");
    drop(conn);

    assert_eq!(
        h.pipeline
            .deliver(std::slice::from_ref(&id))
            .await
            .delivered,
        1
    );
    assert_eq!(delivery_state(&h, &id).await.0, "delivered");
    h.finish().await;
}

#[tokio::test]
async fn the_sweep_grace_runs_from_when_a_row_became_pending() {
    let h = Harness::new("notif_pending_since").await;
    let (id, _) = one_pending(&h).await;
    sqlx::query(
        "UPDATE notifications SET created_at = now() - interval '1 hour' WHERE id = $1::uuid",
    )
    .bind(id.as_str())
    .execute(h.pool.write_pool().as_ref())
    .await
    .expect("age the row");

    let young = h.pipeline.sweep(Utc::now()).await.expect("sweep");
    assert_eq!(young.delivered, 0, "the row only just became pending");
    assert!(h.channel.deliveries().is_empty());

    let due = h
        .pipeline
        .sweep(Utc::now() + chrono::Duration::seconds(60))
        .await
        .expect("sweep");
    assert_eq!(due.delivered, 1);
    h.finish().await;
}

#[tokio::test]
async fn a_row_another_pass_holds_is_skipped_not_announced_twice() {
    let h = Harness::new("notif_delivery_skip_locked").await;
    let (id, _) = one_pending(&h).await;
    let mut holder = h.pool.write_pool().begin().await.expect("begin");
    sqlx::query("SELECT id FROM notifications WHERE id = $1::uuid FOR NO KEY UPDATE")
        .bind(id.as_str())
        .execute(&mut *holder)
        .await
        .expect("hold the row");

    assert_eq!(
        h.pipeline
            .deliver(std::slice::from_ref(&id))
            .await
            .delivered,
        0
    );
    let swept = h
        .pipeline
        .sweep(Utc::now() + chrono::Duration::seconds(60))
        .await
        .expect("sweep");
    assert_eq!(swept.delivered, 0);
    assert!(h.channel.deliveries().is_empty());

    holder.rollback().await.expect("release");
    assert_eq!(
        h.pipeline
            .deliver(std::slice::from_ref(&id))
            .await
            .delivered,
        1
    );
    assert_eq!(h.channel.deliveries().len(), 1);
    h.finish().await;
}

#[tokio::test]
async fn a_transient_channel_failure_leaves_the_row_pending_for_a_retry() {
    let (h, channel) = scripted("notif_channel_retry", &[Step::Unavailable], Step::Deliver).await;
    let (id, _) = one_pending(&h).await;

    h.pipeline.deliver(std::slice::from_ref(&id)).await;
    assert_eq!(delivery_state(&h, &id).await.0, "pending");
    assert_eq!(
        h.channel.deliveries().len(),
        1,
        "the healthy channel delivered"
    );

    h.pipeline.deliver(std::slice::from_ref(&id)).await;
    assert_eq!(delivery_state(&h, &id).await.0, "delivered");
    assert_eq!(channel.calls(), 2);
    assert_eq!(
        h.channel.deliveries().len(),
        1,
        "a surface with a receipt is not asked again"
    );
    assert_eq!(stage_count(&h, "deliver_retry:scripted").await, 1);
    h.finish().await;
}

#[tokio::test]
async fn a_timed_out_channel_is_retried() {
    let (h, channel) = scripted("notif_channel_timeout", &[Step::Hang], Step::Deliver).await;
    let (id, _) = one_pending(&h).await;

    h.pipeline.deliver(std::slice::from_ref(&id)).await;
    assert_eq!(delivery_state(&h, &id).await.0, "pending");
    h.pipeline.deliver(std::slice::from_ref(&id)).await;
    assert_eq!(delivery_state(&h, &id).await.0, "delivered");
    assert_eq!(channel.calls(), 2);
    h.finish().await;
}

#[tokio::test]
async fn a_permanent_channel_failure_closes_the_row_and_is_not_asked_again() {
    let (h, channel) = scripted("notif_channel_permanent", &[], Step::Undeliverable).await;
    let (id, _) = one_pending(&h).await;

    h.pipeline.deliver(std::slice::from_ref(&id)).await;
    assert_eq!(delivery_state(&h, &id).await.0, "delivered");
    assert_eq!(stage_count(&h, "deliver:scripted").await, 1);
    assert_eq!(channel.calls(), 1);
    h.finish().await;
}

#[tokio::test]
async fn transient_channel_failures_are_abandoned_at_the_attempt_cap() {
    let (h, channel) = scripted("notif_channel_abandon", &[], Step::Unavailable).await;
    let (id, _) = one_pending(&h).await;

    for pass in 1..MAX_DELIVERY_ATTEMPTS {
        h.pipeline.deliver(std::slice::from_ref(&id)).await;
        assert_eq!(delivery_state(&h, &id).await.0, "pending", "pass {pass}");
    }
    h.pipeline.deliver(std::slice::from_ref(&id)).await;

    assert_eq!(delivery_state(&h, &id).await.0, "delivered");
    let cap = usize::try_from(MAX_DELIVERY_ATTEMPTS).expect("cap fits");
    assert_eq!(channel.calls(), cap);
    assert_eq!(h.failures(DELIVERY_ABANDONED).await.len(), 1);
    h.finish().await;
}

#[tokio::test]
async fn a_panicking_delivery_task_counts_toward_the_attempt_cap() {
    let (h, _channel) = scripted("notif_channel_panic", &[], Step::Panic).await;
    let (id, _) = one_pending(&h).await;

    for _ in 0..MAX_DELIVERY_ATTEMPTS {
        h.pipeline.deliver(std::slice::from_ref(&id)).await;
    }

    assert_eq!(delivery_state(&h, &id).await.0, "delivered");
    assert_eq!(
        stage_count(&h, "deliver_retry:task").await,
        MAX_DELIVERY_ATTEMPTS
    );
    assert_eq!(h.failures(DELIVERY_ABANDONED).await.len(), 1);
    h.finish().await;
}

#[tokio::test]
async fn no_sweep_runs_while_another_session_holds_the_sweep_lock() {
    let h = Harness::new("notif_sweep_xact_lock").await;
    one_pending(&h).await;
    let later = Utc::now() + chrono::Duration::seconds(60);
    let mut holder = h.pool.write_pool().acquire().await.expect("connection");
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(SWEEP_LOCK_KEY)
        .execute(&mut *holder)
        .await
        .expect("take the sweep lock");

    assert_eq!(h.pipeline.sweep(later).await.expect("sweep").delivered, 0);

    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(SWEEP_LOCK_KEY)
        .execute(&mut *holder)
        .await
        .expect("release the sweep lock");
    assert_eq!(h.pipeline.sweep(later).await.expect("sweep").delivered, 1);

    let free: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(SWEEP_LOCK_KEY)
        .fetch_one(&mut *holder)
        .await
        .expect("probe the sweep lock");
    assert!(free, "the sweep's lock ended with its transaction");
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(SWEEP_LOCK_KEY)
        .execute(&mut *holder)
        .await
        .expect("release the probe");
    drop(holder);
    h.finish().await;
}

#[tokio::test]
async fn the_delivery_schema_carries_sequencing_columns_and_indexes() {
    let h = Harness::new("notif_delivery_schema").await;
    for (table, column) in [
        ("notifications", "delivery_seq"),
        ("notifications", "pending_since"),
        ("notification_delivery_failures", "notification_id"),
    ] {
        let present: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM information_schema.columns \
             WHERE table_name = $1 AND column_name = $2)",
        )
        .bind(table)
        .bind(column)
        .fetch_one(h.pool.write_pool().as_ref())
        .await
        .expect("column probe");
        assert!(present, "{table}.{column}");
    }
    for (index, wanted) in [
        ("idx_notifications_unread_age", true),
        ("idx_notifications_pending_since", true),
        ("idx_notifications_delivery_pending", false),
    ] {
        assert_eq!(
            h.count(
                "SELECT COUNT(*) FROM pg_indexes WHERE indexname = $1",
                index
            )
            .await,
            i64::from(wanted),
            "{index}"
        );
    }
    h.finish().await;
}
