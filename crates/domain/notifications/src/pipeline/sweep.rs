//! [`NotificationPipeline::sweep`]: release the digest-held rows whose
//! window has closed and deliver them together with every row `pending` for
//! longer than the sweep grace, at most one profile batch of each.
//!
//! The whole sweep is one transaction that first takes the sweep's
//! transaction-scoped advisory lock, so one replica sweeps at a time and the
//! lock is released with the transaction however the sweep ends. A replica
//! that does not get the lock sweeps nothing. The grace keys on
//! `pending_since`, so a row the live path has just committed or re-pended
//! is left to it, and the rows' `SKIP LOCKED` row locks keep the sweep and a
//! live pass from announcing one row twice.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::NotificationId;

use super::{DeliverOutcome, NotificationPipeline};
use crate::error::NotificationsResult;
use crate::repository::NotificationRepository;

impl NotificationPipeline {
    #[tracing::instrument(skip_all)]
    pub async fn sweep(&self, now: DateTime<Utc>) -> NotificationsResult<DeliverOutcome> {
        let consumer = &self.parts.profile.consumer;
        let batch = i64::from(consumer.batch.max(1));
        let mut tx = self.parts.repos.write_pool().begin().await?;
        if !NotificationRepository::try_sweep_lock(&mut tx).await? {
            tx.rollback().await?;
            return Ok(DeliverOutcome::default());
        }
        let mut ids = NotificationRepository::release_held(&mut tx, now, batch).await?;
        let grace = chrono::Duration::seconds(i64::from(consumer.sweep_secs));
        ids.extend(NotificationRepository::stale_pending(&mut tx, now - grace, batch).await?);
        let mut seen = HashSet::new();
        let ids: Vec<NotificationId> = ids
            .into_iter()
            .filter(|id| seen.insert(id.clone()))
            .collect();
        let outcome = self.deliver_on(&mut tx, &ids).await?;
        tx.commit().await?;
        Ok(outcome)
    }
}
