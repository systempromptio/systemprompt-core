//! The delivery step, run on committed `pending` rows: hand each row to
//! every channel that delivers its attention and has no `delivered` receipt
//! for it yet, eight rows at a time, each channel call under the profile's
//! channel timeout. A success writes a `delivered` receipt for its surface;
//! a failure or timeout writes a `deliver:<surface>` failure row. Once every
//! eligible channel of a row has one or the other, the row moves to
//! `delivered`; a row whose outcome could not be recorded stays `pending`
//! for the sweep.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use systemprompt_extension::notifications::{ChannelError, NotificationChannel};
use systemprompt_identifiers::{NotificationId, UserId};
use systemprompt_models::notifications::{Notification, ReceiptState};
use tokio::task::JoinSet;

use super::{NotificationPipeline, metrics};
use crate::error::NotificationsResult;
use crate::repository::NotificationFailureRepository;

const CONCURRENCY: usize = 8;

/// What one delivery pass did: channel deliveries that succeeded and failed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeliverOutcome {
    pub delivered: u32,
    pub failed: u32,
}

enum Attempt {
    Delivered,
    Failed(ChannelError),
    TimedOut,
}

struct RowResult {
    id: NotificationId,
    recipient: UserId,
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
        recipient: notification.recipient,
        attempts,
    }
}

impl NotificationPipeline {
    #[tracing::instrument(skip_all, fields(count = ids.len()))]
    pub async fn deliver(&self, ids: &[NotificationId]) -> DeliverOutcome {
        match self.try_deliver(ids).await {
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

    async fn try_deliver(&self, ids: &[NotificationId]) -> NotificationsResult<DeliverOutcome> {
        if ids.is_empty() {
            return Ok(DeliverOutcome::default());
        }
        let repos = &self.parts.repos;
        let rows = repos.notifications.find_pending(ids).await?;
        let done = repos.receipts.delivered_surfaces(ids).await?;
        let recipients: Vec<UserId> = rows
            .iter()
            .map(|n| n.recipient.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let unread: HashMap<UserId, i64> = repos
            .notifications
            .unread_counts(&recipients)
            .await?
            .into_iter()
            .map(|(user, count, _)| (user, count))
            .collect();
        let timeout = Duration::from_millis(self.parts.profile.consumer.channel_timeout_ms);
        let mut set: JoinSet<RowResult> = JoinSet::new();
        let mut results = Vec::with_capacity(rows.len());
        for notification in rows {
            while set.len() >= CONCURRENCY {
                if let Some(joined) = set.join_next().await {
                    results.push(joined);
                }
            }
            let delivered = done.get(&notification.id);
            let eligible: Vec<Arc<dyn NotificationChannel>> = self
                .parts
                .channels
                .iter()
                .filter(|c| c.delivers(notification.attention))
                .filter(|c| !delivered.is_some_and(|s| s.contains(c.surface())))
                .cloned()
                .collect();
            let count = unread.get(&notification.recipient).copied().unwrap_or(0);
            set.spawn(deliver_row(eligible, notification, count, timeout));
        }
        while let Some(joined) = set.join_next().await {
            results.push(joined);
        }
        self.record(results).await
    }

    async fn record(
        &self,
        results: Vec<Result<RowResult, tokio::task::JoinError>>,
    ) -> NotificationsResult<DeliverOutcome> {
        let repos = &self.parts.repos;
        let mut outcome = DeliverOutcome::default();
        let mut receipts: BTreeMap<&'static str, Vec<NotificationId>> = BTreeMap::new();
        let mut failures: Vec<(Option<UserId>, String, String)> = Vec::new();
        let mut finished = Vec::new();
        for joined in results {
            let row = match joined {
                Ok(row) => row,
                Err(error) => {
                    tracing::error!(error = %error, "Notification delivery task failed");
                    continue;
                },
            };
            for (surface, attempt) in row.attempts {
                let reason = match attempt {
                    Attempt::Delivered => {
                        metrics::delivery(surface, "delivered");
                        outcome.delivered += 1;
                        receipts.entry(surface).or_default().push(row.id.clone());
                        continue;
                    },
                    Attempt::Failed(error) => {
                        tracing::warn!(notification_id = %row.id, surface, error = %error, "Notification channel delivery failed");
                        metrics::delivery(surface, "failed");
                        error.to_string()
                    },
                    Attempt::TimedOut => {
                        tracing::warn!(notification_id = %row.id, surface, "Notification channel delivery timed out");
                        metrics::delivery(surface, "timeout");
                        "channel_timeout".to_owned()
                    },
                };
                outcome.failed += 1;
                failures.push((
                    Some(row.recipient.clone()),
                    format!("deliver:{surface}"),
                    reason,
                ));
            }
            finished.push(row.id);
        }
        for (surface, ids) in receipts {
            repos
                .receipts
                .record_detached(&ids, surface, ReceiptState::Delivered)
                .await?;
        }
        NotificationFailureRepository::record_many(repos.write_pool(), None, &failures).await?;
        repos.notifications.mark_delivered(&finished).await?;
        Ok(outcome)
    }
}
