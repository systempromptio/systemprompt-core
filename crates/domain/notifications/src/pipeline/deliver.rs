//! The delivery step, run on committed `pending` rows with no transaction
//! open while channels run. One statement claims the rows under a
//! [`DeliveryLease`] (a row another pass holds a live lease on is left to
//! it); the receipts, failure history and unread counts are read on the
//! pool; then each row is handed to every channel that delivers its
//! attention and has neither a `delivered` receipt nor a permanent failure
//! for it, eight rows at a time, each channel call under the profile's
//! channel timeout. Each row's outcome is then written in its own short
//! transaction that first finishes the claim: when the lease was lost the
//! transaction writes nothing. The [`Ledger`] decides whether the row closes;
//! a row is marked `delivered` only at the `delivery_seq` it was claimed at.
//! A deliverer that stops mid-pass leaves its claims to expire, after which
//! the sweep delivers them again, so delivery is at least once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use systemprompt_extension::notifications::NotificationChannel;
use systemprompt_identifiers::{NotificationId, UserId};
use systemprompt_models::notifications::{Notification, ReceiptState};
use tokio::task::{JoinError, JoinSet};

use super::NotificationPipeline;
use super::ledger::{Attempt, Ledger, LockedRow};
use crate::error::NotificationsResult;
use crate::repository::{
    DeliveryHistory, DeliveryLease, Fenced, NotificationFailureRepository,
    NotificationReceiptRepository, NotificationRepository,
};

const CONCURRENCY: usize = 8;
const LEASE_SLACK: Duration = Duration::from_secs(30);

/// What one delivery pass did: channel deliveries that succeeded and failed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeliverOutcome {
    pub delivered: u32,
    pub failed: u32,
}

struct RowResult {
    id: NotificationId,
    attempts: Vec<(&'static str, Attempt)>,
}

struct Context {
    rows: Vec<Notification>,
    done: HashMap<NotificationId, HashSet<String>>,
    history: HashMap<NotificationId, DeliveryHistory>,
    unread: HashMap<UserId, i64>,
}

struct Claimed {
    lease: DeliveryLease,
    row: LockedRow,
}

async fn deliver_row(
    channels: Vec<Arc<dyn NotificationChannel>>,
    notification: Notification,
    unread: i64,
    timeout: Duration,
) -> RowResult {
    let mut attempts = Vec::with_capacity(channels.len());
    for channel in channels {
        let surface = channel.surface();
        let attempt =
            match tokio::time::timeout(timeout, channel.deliver(&notification, unread)).await {
                Ok(Ok(())) => Attempt::Delivered,
                Ok(Err(error)) => Attempt::Failed(error),
                Err(_) => Attempt::TimedOut,
            };
        attempts.push((surface, attempt));
    }
    RowResult {
        id: notification.id,
        attempts,
    }
}

fn lease_for(timeout: Duration, channels: usize, rows: usize) -> Duration {
    let rounds = u32::try_from(rows.div_ceil(CONCURRENCY)).unwrap_or(u32::MAX);
    let per_row = timeout.saturating_mul(u32::try_from(channels.max(1)).unwrap_or(u32::MAX));
    per_row.saturating_mul(rounds).saturating_add(LEASE_SLACK)
}

impl NotificationPipeline {
    #[tracing::instrument(skip_all, fields(count = ids.len()))]
    pub async fn deliver(&self, ids: &[NotificationId]) -> DeliverOutcome {
        match self.deliver_claimed(ids).await {
            Ok(outcome) => outcome,
            Err(error) => {
                tracing::error!(error = %error, count = ids.len(), "Notification delivery could not be recorded");
                DeliverOutcome {
                    delivered: 0,
                    failed: u32::try_from(ids.len()).unwrap_or(u32::MAX),
                }
            },
        }
    }

    async fn deliver_claimed(&self, ids: &[NotificationId]) -> NotificationsResult<DeliverOutcome> {
        if ids.is_empty() {
            return Ok(DeliverOutcome::default());
        }
        let repos = &self.parts.repos;
        let timeout = Duration::from_millis(self.parts.profile.consumer.channel_timeout_ms);
        let leases = repos
            .notifications
            .claim_pending(
                ids,
                lease_for(timeout, self.parts.channels.len(), ids.len()),
            )
            .await?;
        if leases.is_empty() {
            return Ok(DeliverOutcome::default());
        }
        let claimed_ids: Vec<NotificationId> = leases.iter().map(|l| l.id().clone()).collect();
        let Context {
            rows,
            done,
            mut history,
            unread,
        } = self.context(&claimed_ids).await?;
        let mut leases: HashMap<NotificationId, DeliveryLease> =
            leases.into_iter().map(|l| (l.id().clone(), l)).collect();
        let mut set: JoinSet<RowResult> = JoinSet::new();
        let mut owners: HashMap<tokio::task::Id, NotificationId> = HashMap::new();
        let mut claimed: HashMap<NotificationId, Claimed> = HashMap::new();
        let mut results = Vec::with_capacity(rows.len());
        for notification in rows {
            let Some(lease) = leases.remove(&notification.id) else {
                continue;
            };
            while set.len() >= CONCURRENCY {
                if let Some(joined) = set.join_next().await {
                    results.push(joined);
                }
            }
            let past = history.remove(&notification.id).unwrap_or_default();
            let delivered = done.get(&notification.id);
            let eligible: Vec<Arc<dyn NotificationChannel>> = self
                .parts
                .channels
                .iter()
                .filter(|c| c.delivers(notification.attention))
                .filter(|c| !delivered.is_some_and(|s| s.contains(c.surface())))
                .filter(|c| !past.permanent.contains(c.surface()))
                .cloned()
                .collect();
            let count = unread.get(&notification.recipient).copied().unwrap_or(0);
            let id = notification.id.clone();
            claimed.insert(
                id.clone(),
                Claimed {
                    row: LockedRow {
                        recipient: notification.recipient.clone(),
                        attempts: past.attempts,
                    },
                    lease,
                },
            );
            let task = set.spawn(deliver_row(eligible, notification, count, timeout));
            owners.insert(task.id(), id);
        }
        while let Some(joined) = set.join_next().await {
            results.push(joined);
        }
        Ok(self.record(results, &owners, &claimed).await)
    }

    async fn context(&self, ids: &[NotificationId]) -> NotificationsResult<Context> {
        let repos = &self.parts.repos;
        let rows = repos.notifications.load_claimed(ids).await?;
        let pool = repos.write_pool();
        let done = NotificationReceiptRepository::delivered_surfaces(pool, ids).await?;
        let history = NotificationFailureRepository::delivery_history(pool, ids).await?;
        let recipients: Vec<UserId> = rows
            .iter()
            .map(|n| n.recipient.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let unread = repos
            .notifications
            .unread_counts(&recipients)
            .await?
            .into_iter()
            .map(|(user, count, _)| (user, count))
            .collect();
        Ok(Context {
            rows,
            done,
            history,
            unread,
        })
    }

    async fn record(
        &self,
        results: Vec<Result<RowResult, JoinError>>,
        owners: &HashMap<tokio::task::Id, NotificationId>,
        claimed: &HashMap<NotificationId, Claimed>,
    ) -> DeliverOutcome {
        let mut outcome = DeliverOutcome::default();
        for joined in results {
            let mut ledger = Ledger::default();
            let entry = match joined {
                Ok(result) => {
                    let Some(entry) = claimed.get(&result.id) else {
                        continue;
                    };
                    ledger.row(&result.id, &entry.row, result.attempts);
                    entry
                },
                Err(error) => {
                    let owned = owners.get(&error.id()).and_then(|id| claimed.get(id));
                    let Some(entry) = owned else {
                        tracing::error!(error = %error, "Notification delivery task failed");
                        continue;
                    };
                    let id = entry.lease.id();
                    tracing::error!(notification_id = %id, error = %error, "Notification delivery task failed");
                    ledger.task_failed(id, &entry.row, error.to_string());
                    entry
                },
            };
            match self.write_row(&entry.lease, &ledger).await {
                Ok(Fenced::Applied) => {
                    outcome.delivered += ledger.outcome.delivered;
                    outcome.failed += ledger.outcome.failed;
                },
                Ok(Fenced::LeaseLost) => {
                    tracing::warn!(notification_id = %entry.lease.id(), "Notification delivery lease lost; outcome not recorded");
                },
                Err(error) => {
                    tracing::error!(notification_id = %entry.lease.id(), error = %error, "Notification delivery outcome could not be recorded; the claim expires for the sweep");
                    outcome.failed += ledger.outcome.delivered + ledger.outcome.failed;
                },
            }
        }
        outcome
    }

    async fn write_row(
        &self,
        lease: &DeliveryLease,
        ledger: &Ledger,
    ) -> NotificationsResult<Fenced> {
        let mut tx = self.parts.repos.write_pool().begin().await?;
        if NotificationRepository::finish_claim(&mut tx, lease, ledger.close).await?
            == Fenced::LeaseLost
        {
            tx.rollback().await?;
            return Ok(Fenced::LeaseLost);
        }
        for (surface, ids) in &ledger.receipts {
            NotificationReceiptRepository::record(&mut *tx, ids, surface, ReceiptState::Delivered)
                .await?;
        }
        NotificationFailureRepository::record_deliveries(&mut *tx, &ledger.failures).await?;
        tx.commit().await?;
        Ok(Fenced::Applied)
    }
}
