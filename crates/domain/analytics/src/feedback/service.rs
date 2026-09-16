//! Owned, cancellable workers resume committed changes after process restart.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::FeedbackFactsRepository;
use crate::Result;
use systemprompt_identifiers::{AnalyticsWorkerId, UserId};

#[derive(Debug, Clone)]
pub struct FactsProcessingService {
    repository: FeedbackFactsRepository,
}

impl FactsProcessingService {
    pub const fn new(repository: FeedbackFactsRepository) -> Self {
        Self { repository }
    }

    pub async fn drain(
        &self,
        owner: &UserId,
        worker: &AnalyticsWorkerId,
        limit: u32,
    ) -> Result<usize> {
        let leases = self.repository.claim(owner, worker, limit, 60).await?;
        let mut completed = 0;
        for lease in leases {
            match self.repository.apply(owner, &lease).await {
                Ok(_) => completed += 1,
                Err(_error) => {
                    if self.repository.retry(owner, &lease).await.is_err() {
                        tracing::warn!(change_id = %lease.change_id, "Analytics lease lost; current lease owner will resume processing");
                    }
                },
            }
        }
        Ok(completed)
    }

    pub async fn run(
        &self,
        owner: &UserId,
        worker: &AnalyticsWorkerId,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<()> {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        while !*shutdown.borrow() {
            tokio::select! {
                changed = shutdown.changed() => { if changed.is_err() || *shutdown.borrow() { break; } },
                _ = interval.tick() => {
                    if self.drain(owner, worker, 64).await.is_err() {
                        tracing::warn!(worker_id = %worker, "Analytics worker storage unavailable; committed changes remain pending");
                    }
                },
            }
        }
        Ok(())
    }
}
