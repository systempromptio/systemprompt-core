use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use systemprompt_extension::notifications::{ChannelError, NotificationChannel};
use systemprompt_identifiers::{DocumentId, NotificationId};
use systemprompt_models::notifications::{Attention, Notification};

use crate::harness::{Harness, delivery_state, mention, profile};

#[derive(Clone, Copy)]
enum Probe {
    Observe,
    StealLease,
}

struct Observed {
    open_transactions: i64,
    row_lockable: bool,
}

struct Probing {
    pool: OnceLock<PgPool>,
    probe: Probe,
    seen: Mutex<Vec<Observed>>,
}

impl Probing {
    fn pool(&self) -> &PgPool {
        self.pool.get().expect("pool installed")
    }

    async fn observe(&self, id: &NotificationId) -> Observed {
        let open_transactions = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM pg_stat_activity WHERE datname = current_database() \
             AND pid <> pg_backend_pid() AND state LIKE 'idle in transaction%'",
        )
        .fetch_one(self.pool())
        .await
        .expect("activity");
        let mut tx = self.pool().begin().await.expect("begin");
        let row_lockable =
            sqlx::query("SELECT id FROM notifications WHERE id = $1::uuid FOR UPDATE NOWAIT")
                .bind(id.as_str())
                .fetch_one(&mut *tx)
                .await
                .is_ok();
        tx.rollback().await.expect("rollback");
        Observed {
            open_transactions,
            row_lockable,
        }
    }
}

#[async_trait]
impl NotificationChannel for Probing {
    fn surface(&self) -> &'static str {
        "probing"
    }

    fn delivers(&self, _attention: Attention) -> bool {
        true
    }

    async fn deliver(&self, notification: &Notification, _unread: i64) -> Result<(), ChannelError> {
        match self.probe {
            Probe::Observe => {
                let observed = self.observe(&notification.id).await;
                self.seen.lock().expect("lock").push(observed);
            },
            Probe::StealLease => {
                sqlx::query(
                    "UPDATE notifications SET delivery_lease_token = gen_random_uuid() \
                     WHERE id = $1::uuid",
                )
                .bind(notification.id.as_str())
                .execute(self.pool())
                .await
                .expect("steal lease");
            },
        }
        Ok(())
    }
}

async fn probing(prefix: &str, probe: Probe) -> (Harness, Arc<Probing>) {
    let channel = Arc::new(Probing {
        pool: OnceLock::new(),
        probe,
        seen: Mutex::new(Vec::new()),
    });
    let seam = Arc::clone(&channel);
    let h = Harness::with_seams(prefix, profile(), move |parts| {
        parts.channels = vec![seam as Arc<dyn NotificationChannel>];
    })
    .await;
    channel
        .pool
        .set(h.pool.write_pool().as_ref().clone())
        .expect("pool set once");
    (h, channel)
}

async fn one_pending(h: &Harness) -> NotificationId {
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let (event, _) = mention(&author, &reader, &DocumentId::generate(), Utc::now());
    h.emit(&event).await;
    let processed = h.process_pending().await;
    assert_eq!(processed.pending.len(), 1);
    processed.pending[0].clone()
}

async fn lease_token(h: &Harness, id: &NotificationId) -> Option<String> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT delivery_lease_token::text FROM notifications WHERE id = $1::uuid",
    )
    .bind(id.as_str())
    .fetch_one(h.pool.write_pool().as_ref())
    .await
    .expect("lease token")
}

async fn set_lease_until(h: &Harness, id: &NotificationId, offset_secs: i64) {
    sqlx::query(
        "UPDATE notifications SET delivery_lease_token = gen_random_uuid(), \
         delivery_lease_until = now() + make_interval(secs => $2) WHERE id = $1::uuid",
    )
    .bind(id.as_str())
    .bind(offset_secs as f64)
    .execute(h.pool.write_pool().as_ref())
    .await
    .expect("set lease");
}

#[tokio::test]
async fn a_channel_call_runs_with_no_transaction_open_and_the_row_unlocked() {
    let (h, channel) = probing("notif_lease_no_tx", Probe::Observe).await;
    let id = one_pending(&h).await;

    let outcome = h.pipeline.deliver(std::slice::from_ref(&id)).await;

    assert_eq!(outcome.delivered, 1);
    let seen = channel
        .seen
        .lock()
        .expect("lock")
        .drain(..)
        .collect::<Vec<_>>();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        seen[0].open_transactions, 0,
        "no connection sits in a transaction while a channel runs"
    );
    assert!(seen[0].row_lockable, "the delivered row is not row-locked");
    assert_eq!(delivery_state(&h, &id).await.0, "delivered");
    assert_eq!(lease_token(&h, &id).await, None);
    h.finish().await;
}

#[tokio::test]
async fn a_pass_whose_lease_was_taken_over_records_nothing() {
    let (h, _channel) = probing("notif_lease_lost", Probe::StealLease).await;
    let id = one_pending(&h).await;

    let outcome = h.pipeline.deliver(std::slice::from_ref(&id)).await;

    assert_eq!(outcome.delivered, 0);
    assert_eq!(h.receipts("delivered").await, 0);
    assert_eq!(delivery_state(&h, &id).await.0, "pending");
    assert!(
        lease_token(&h, &id).await.is_some(),
        "the new holder's lease is left in place"
    );
    h.finish().await;
}

#[tokio::test]
async fn a_live_claim_is_skipped_and_an_expired_one_is_delivered_by_the_sweep() {
    let (h, _channel) = probing("notif_lease_expiry", Probe::Observe).await;
    let id = one_pending(&h).await;

    set_lease_until(&h, &id, 3600).await;
    let live = h
        .pipeline
        .sweep(Utc::now() + Duration::seconds(60))
        .await
        .expect("sweep");
    assert_eq!(live.delivered, 0, "a live claim belongs to its holder");
    assert_eq!(
        h.pipeline
            .deliver(std::slice::from_ref(&id))
            .await
            .delivered,
        0
    );

    set_lease_until(&h, &id, -1).await;
    let expired = h
        .pipeline
        .sweep(Utc::now() + Duration::seconds(60))
        .await
        .expect("sweep");
    assert_eq!(expired.delivered, 1);
    assert_eq!(delivery_state(&h, &id).await.0, "delivered");
    assert_eq!(lease_token(&h, &id).await, None);
    h.finish().await;
}
