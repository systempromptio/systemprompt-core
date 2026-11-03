//! [`BrainPipeline::sweep`]: the periodic pass over the store. It holds
//! the brain's transaction-scoped sweep lock so one replica sweeps at a
//! time; it reports the store's fact counts, open disputes and the day's
//! extraction tokens as metrics, and checks the working-set projection's
//! row count against the count the facts and topics imply, rebuilding it
//! (`brain_working_set_rebuilds_total`) when they disagree, drains the
//! `brain_fact_index` projection backlog an upgrade queued (at most
//! ten seconds per sweep), and recounts the search term
//! statistics the full-text tier plans with.
//!
//! [`BrainPipeline::refresh_store_gauges`] republishes the fact and open
//! dispute gauges between sweeps, after a drain round that wrote; it takes
//! no lock, since the counts are read-only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use chrono::{DateTime, Utc};
use systemprompt_database::projection::{self, DrainReport};

use super::{BrainPipeline, metrics};
use crate::error::BrainResult;
use crate::index::{self, BRAIN_FACT_INDEX};
use crate::repository::{
    BrainLocks, FactCounts, SearchTermsRepository, WorkingSetCounts, WorkingSetRepository,
};

/// What one sweep observed; `None` from [`BrainPipeline::sweep`] means
/// another replica held the lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepReport {
    pub facts: FactCounts,
    pub open_disputes: i64,
    pub tokens_today: i64,
    pub working_set: WorkingSetCounts,
    pub working_set_rebuilt: bool,
    pub index_backlog: DrainReport,
}

/// How long one sweep spends draining queued fact keys; the rest wait for
/// the next sweep.
const INDEX_DRAIN_BUDGET: Duration = Duration::from_secs(10);
const INDEX_DRAIN_BATCH: i32 = 256;

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
        let index_backlog = projection::drain(
            self.repos.write_pool(),
            BRAIN_FACT_INDEX,
            INDEX_DRAIN_BATCH,
            INDEX_DRAIN_BUDGET,
            |conn, keys| Box::pin(index::refresh_projection(conn, keys)),
        )
        .await?;
        let working_set = WorkingSetRepository::counts(&mut *tx).await?;
        let working_set_rebuilt = !working_set.is_consistent();
        if working_set_rebuilt {
            WorkingSetRepository::rebuild_all(&mut tx).await?;
            metrics::working_set_rebuilt();
        }
        SearchTermsRepository::refresh(&mut tx).await?;
        let report = SweepReport {
            facts: self.repos.facts.counts().await?,
            open_disputes: self.repos.disputes.count_open().await?,
            tokens_today: self.repos.extractions.tokens_used_since(day_start).await?,
            working_set,
            working_set_rebuilt,
            index_backlog,
        };
        metrics::store(&report.facts, report.open_disputes, report.tokens_today);
        tx.commit().await?;
        Ok(Some(report))
    }

    /// Republishes `brain_facts{status}` and `brain_disputes_open` from the
    /// store.
    pub async fn refresh_store_gauges(&self) -> BrainResult<()> {
        let facts = self.repos.facts.counts().await?;
        let open_disputes = self.repos.disputes.count_open().await?;
        metrics::store_counts(&facts, open_disputes);
        Ok(())
    }
}
