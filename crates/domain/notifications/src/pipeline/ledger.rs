//! [`Ledger`]: what one delivery pass writes back, accumulated per row.
//!
//! A channel success is a `delivered` receipt for its surface. A permanent
//! channel failure is a `deliver:<surface>` row and that surface is never
//! asked again for the notification; a timeout, a retryable channel error or
//! a failed delivery task is a `deliver_retry:<surface>` row and leaves the
//! notification `pending` for the sweep. A row closes as `delivered` once no
//! eligible channel is left to retry, or when this pass is its
//! [`MAX_DELIVERY_ATTEMPTS`]th with a transient failure, which also records
//! a final `delivery_abandoned` row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_extension::notifications::ChannelError;
use systemprompt_identifiers::{NotificationId, UserId};

use super::{DeliverOutcome, metrics};
use crate::repository::{DeliveryFailure, FailureStage};

pub const MAX_DELIVERY_ATTEMPTS: i64 = 5;
pub const DELIVERY_ABANDONED: &str = "delivery_abandoned";
const TASK_SURFACE: &str = "task";

pub(super) enum Attempt {
    Delivered,
    Failed(ChannelError),
    TimedOut,
}

pub(super) struct LockedRow {
    pub(super) recipient: UserId,
    pub(super) seq: i64,
    pub(super) attempts: i64,
}

#[derive(Default)]
pub(super) struct Ledger {
    pub(super) outcome: DeliverOutcome,
    pub(super) receipts: BTreeMap<&'static str, Vec<NotificationId>>,
    pub(super) failures: Vec<DeliveryFailure>,
    pub(super) finished: Vec<(NotificationId, i64)>,
}

impl Ledger {
    pub(super) fn row(
        &mut self,
        id: &NotificationId,
        row: &LockedRow,
        attempts: Vec<(&'static str, Attempt)>,
    ) {
        let mut retry = false;
        for (surface, attempt) in attempts {
            let (stage, reason) = match attempt {
                Attempt::Delivered => {
                    metrics::delivery(surface, "delivered");
                    self.outcome.delivered += 1;
                    self.receipts.entry(surface).or_default().push(id.clone());
                    continue;
                },
                Attempt::Failed(error) if !error.is_retryable() => {
                    tracing::warn!(notification_id = %id, surface, error = %error, "Notification channel delivery failed permanently");
                    metrics::delivery(surface, "failed");
                    (
                        FailureStage::Deliver {
                            surface: surface.to_owned(),
                        },
                        error.to_string(),
                    )
                },
                Attempt::Failed(error) => {
                    tracing::warn!(notification_id = %id, surface, error = %error, "Notification channel delivery failed");
                    metrics::delivery(surface, "failed");
                    retry = true;
                    (
                        FailureStage::DeliverRetry {
                            surface: surface.to_owned(),
                        },
                        error.to_string(),
                    )
                },
                Attempt::TimedOut => {
                    tracing::warn!(notification_id = %id, surface, "Notification channel delivery timed out");
                    metrics::delivery(surface, "timeout");
                    retry = true;
                    (
                        FailureStage::DeliverRetry {
                            surface: surface.to_owned(),
                        },
                        "channel_timeout".to_owned(),
                    )
                },
            };
            self.fail(id, row, stage, reason);
        }
        self.close(id, row, retry);
    }

    pub(super) fn task_failed(&mut self, id: &NotificationId, row: &LockedRow, reason: String) {
        metrics::delivery(TASK_SURFACE, "failed");
        self.fail(
            id,
            row,
            FailureStage::DeliverRetry {
                surface: TASK_SURFACE.to_owned(),
            },
            reason,
        );
        self.close(id, row, true);
    }

    fn fail(&mut self, id: &NotificationId, row: &LockedRow, stage: FailureStage, reason: String) {
        self.outcome.failed += 1;
        self.failures.push(DeliveryFailure {
            notification: id.clone(),
            recipient: row.recipient.clone(),
            stage,
            reason,
        });
    }

    fn close(&mut self, id: &NotificationId, row: &LockedRow, retry: bool) {
        if !retry {
            self.finished.push((id.clone(), row.seq));
            return;
        }
        if row.attempts + 1 >= MAX_DELIVERY_ATTEMPTS {
            tracing::warn!(notification_id = %id, attempts = row.attempts + 1, "Notification delivery abandoned");
            self.failures.push(DeliveryFailure {
                notification: id.clone(),
                recipient: row.recipient.clone(),
                stage: FailureStage::DeliverAbandoned,
                reason: DELIVERY_ABANDONED.to_owned(),
            });
            self.finished.push((id.clone(), row.seq));
        }
    }
}
