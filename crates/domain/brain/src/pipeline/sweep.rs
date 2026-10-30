//! [`BrainPipeline::sweep`]: the periodic pass over the store. It holds
//! the brain's transaction-scoped sweep lock so one replica sweeps at a
//! time; today it reports the store's fact counts, open disputes and the
//! day's extraction tokens as metrics, and the lifecycle jobs may extend it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};

use super::{BrainPipeline, metrics};
use crate::error::BrainResult;
use crate::repository::{BrainLocks, FactCounts};

/// What one sweep observed; `None` from [`BrainPipeline::sweep`] means
/// another replica held the lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepReport {
    pub facts: FactCounts,
    pub open_disputes: i64,
    pub tokens_today: i64,
}

impl BrainPipeline {
    #[tracing::instrument(skip_all)]
    pub async fn sweep(&self, now: DateTime<Utc>) -> BrainResult<Option<SweepReport>> {
        let mut tx = self.repos.write_pool().begin().await?;
        if !BrainLocks::try_sweep(&mut tx).await? {
            tx.rollback().await?;
            return Ok(None);
        }
        let day_start = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .map_or(now, |midnight| midnight.and_utc());
        let report = SweepReport {
            facts: self.repos.facts.counts().await?,
            open_disputes: self.repos.disputes.count_open().await?,
            tokens_today: self.repos.extractions.tokens_used_since(day_start).await?,
        };
        metrics::store(&report.facts, report.open_disputes, report.tokens_today);
        tx.commit().await?;
        Ok(Some(report))
    }
}
