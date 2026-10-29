//! The delivery step, run on committed `pending` rows inside one
//! transaction: lock the rows still `pending` with `SKIP LOCKED` (a row
//! another pass holds is left to it), hand each row to every channel that
//! delivers its attention and has neither a `delivered` receipt nor a
//! permanent failure for it, eight rows at a time, each channel call under
//! the profile's channel timeout, then write the pass's receipts, failures
//! and closed rows on the same transaction. The [`Ledger`] decides which
//! rows close; a row is marked `delivered` only at the `delivery_seq` it was
//! locked at. A pass whose outcome cannot be written rolls back and leaves
//! its rows `pending` for the sweep.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgConnection;
use systemprompt_extension::notifications::NotificationChannel;
use systemprompt_identifiers::{NotificationId, UserId};
use systemprompt_models::notifications::{Notification, ReceiptState};
use tokio::task::{JoinError, JoinSet};

use super::NotificationPipeline;
use super::ledger::{Attempt, Ledger, LockedRow};
use crate::error::NotificationsResult;
use crate::repository::{
    NotificationFailureRepository, NotificationReceiptRepository, NotificationRepository,
};

const CONCURRENCY: usize = 8;

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

impl NotificationPipeline {
    #[tracing::instrument(skip_all, fields(count = ids.len()))]
    pub async fn deliver(&self, ids: &[NotificationId]) -> DeliverOutcome {
        match self.deliver_in_transaction(ids).await {
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

    async fn deliver_in_transaction(
        &self,
        ids: &[NotificationId],
    ) -> NotificationsResult<DeliverOutcome> {
        if ids.is_empty() {
            return Ok(DeliverOutcome::default());
        }
        let mut tx = self.parts.repos.write_pool().begin().await?;
        let outcome = self.deliver_on(&mut tx, ids).await?;
        tx.commit().await?;
        Ok(outcome)
    }

    pub(super) async fn deliver_on(
        &self,
        conn: &mut PgConnection,
        ids: &[NotificationId],
    ) -> NotificationsResult<DeliverOutcome> {
        if ids.is_empty() {
            return Ok(DeliverOutcome::default());
        }
        let rows = NotificationRepository::lock_pending(&mut *conn, ids).await?;
        if rows.is_empty() {
            return Ok(DeliverOutcome::default());
        }
        let locked_ids: Vec<NotificationId> = rows.iter().map(|(n, _)| n.id.clone()).collect();
        let done =
            NotificationReceiptRepository::delivered_surfaces(&mut *conn, &locked_ids).await?;
        let mut history =
            NotificationFailureRepository::delivery_history(&mut *conn, &locked_ids).await?;
        let recipients: Vec<UserId> = rows
            .iter()
            .map(|(n, _)| n.recipient.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let unread: HashMap<UserId, i64> =
            NotificationRepository::unread_counts_on(&mut *conn, &recipients)
                .await?
                .into_iter()
                .map(|(user, count, _)| (user, count))
                .collect();
        let timeout = Duration::from_millis(self.parts.profile.consumer.channel_timeout_ms);
        let mut set: JoinSet<RowResult> = JoinSet::new();
        let mut owners: HashMap<tokio::task::Id, NotificationId> = HashMap::new();
        let mut locked: HashMap<NotificationId, LockedRow> = HashMap::new();
        let mut results = Vec::with_capacity(rows.len());
        for (notification, seq) in rows {
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
            locked.insert(
                id.clone(),
                LockedRow {
                    recipient: notification.recipient.clone(),
                    seq,
                    attempts: past.attempts,
                },
            );
            let task = set.spawn(deliver_row(eligible, notification, count, timeout));
            owners.insert(task.id(), id);
        }
        while let Some(joined) = set.join_next().await {
            results.push(joined);
        }
        record(conn, results, &owners, &locked).await
    }
}

async fn record(
    conn: &mut PgConnection,
    results: Vec<Result<RowResult, JoinError>>,
    owners: &HashMap<tokio::task::Id, NotificationId>,
    locked: &HashMap<NotificationId, LockedRow>,
) -> NotificationsResult<DeliverOutcome> {
    let mut ledger = Ledger::default();
    for joined in results {
        match joined {
            Ok(result) => {
                if let Some(row) = locked.get(&result.id) {
                    ledger.row(&result.id, row, result.attempts);
                }
            },
            Err(error) => {
                let owned = owners
                    .get(&error.id())
                    .and_then(|id| locked.get(id).map(|row| (id, row)));
                let Some((id, row)) = owned else {
                    tracing::error!(error = %error, "Notification delivery task failed");
                    continue;
                };
                tracing::error!(notification_id = %id, error = %error, "Notification delivery task failed");
                ledger.task_failed(id, row, error.to_string());
            },
        }
    }
    for (surface, ids) in &ledger.receipts {
        NotificationReceiptRepository::record(&mut *conn, ids, surface, ReceiptState::Delivered)
            .await?;
    }
    NotificationFailureRepository::record_deliveries(&mut *conn, &ledger.failures).await?;
    NotificationRepository::mark_delivered(&mut *conn, &ledger.finished).await?;
    Ok(ledger.outcome)
}
