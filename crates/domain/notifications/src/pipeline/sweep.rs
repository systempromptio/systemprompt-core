//! [`NotificationPipeline::sweep`]: release the digest-held rows whose
//! window has closed and deliver them together with every `pending` row
//! older than the sweep grace, at most one profile batch of each. The
//! consumer runs it on a timer under an advisory lock, so one replica sweeps
//! at a time; a row the live path is still announcing is younger than the
//! grace and is left to it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::NotificationId;

use super::{DeliverOutcome, NotificationPipeline};
use crate::error::NotificationsResult;

impl NotificationPipeline {
    #[tracing::instrument(skip_all)]
    pub async fn sweep(&self, now: DateTime<Utc>) -> NotificationsResult<DeliverOutcome> {
        let consumer = &self.parts.profile.consumer;
        let batch = i64::from(consumer.batch.max(1));
        let notifications = &self.parts.repos.notifications;
        let mut ids = notifications.release_held(now, batch).await?;
        let grace = chrono::Duration::seconds(i64::from(consumer.sweep_secs));
        ids.extend(notifications.stale_pending(now - grace, batch).await?);
        let mut seen = HashSet::new();
        let ids: Vec<NotificationId> = ids
            .into_iter()
            .filter(|id| seen.insert(id.clone()))
            .collect();
        Ok(self.deliver(&ids).await)
    }
}
