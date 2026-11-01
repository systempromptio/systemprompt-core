//! [`BrainRetentionJob`]: every ten minutes, bounded batches under a time
//! budget each:
//!
//! - retired facts not under legal hold, untouched for
//!   `lifecycle.retired_retention_days`, are deleted;
//! - recalls older than `lifecycle.recalls_retention_days` are deleted;
//! - per-event results, failures, extractions and outcome signals older than
//!   `lifecycle.results_retention_days` are deleted;
//! - facts past their `retention_until` and not under legal hold are taken out
//!   of recall as superseded, one revision each.
//!
//! Whatever is left waits for the next run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_manifest::profile::{BrainLifecycleProfile, BrainProfile};
use systemprompt_provider_contracts::{JobScope, submit_job};
use systemprompt_traits::{
    Job, JobContext, JobResult, ProviderError, ProviderResult, RepositoryError,
};

use crate::models::FactChange;
use crate::repository::{BrainRepositories, FactRepository};

const BATCH: i64 = 1000;
const BATCH_LEN: usize = 1000;
const BUDGET: Duration = Duration::from_secs(10);

/// Deletes and retires brain rows past their retention.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrainRetentionJob;

async fn drain<F, Fut>(mut step: F) -> Result<u64, RepositoryError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<u64, RepositoryError>>,
{
    let started = Instant::now();
    let mut purged = 0;
    while started.elapsed() < BUDGET {
        let deleted = step().await?;
        purged += deleted;
        if deleted < BATCH.unsigned_abs() {
            break;
        }
    }
    Ok(purged)
}

fn cutoff(days: u32) -> DateTime<Utc> {
    Utc::now() - chrono::Duration::days(i64::from(days))
}

impl BrainRetentionJob {
    async fn purge(
        repos: &BrainRepositories,
        lifecycle: &BrainLifecycleProfile,
    ) -> Result<u64, RepositoryError> {
        let retired = cutoff(lifecycle.retired_retention_days);
        let facts = drain(|| repos.retention.delete_retired_before(retired, BATCH)).await?;
        let recalls_cutoff = cutoff(lifecycle.recalls_retention_days);
        let recalls = drain(|| repos.recalls.purge_before(recalls_cutoff, BATCH)).await?;
        let results_cutoff = cutoff(lifecycle.results_retention_days);
        let results = drain(|| {
            repos
                .retention
                .purge_event_results_before(results_cutoff, BATCH)
        })
        .await?;
        let failures =
            drain(|| repos.retention.purge_failures_before(results_cutoff, BATCH)).await?;
        let extractions = drain(|| {
            repos
                .retention
                .purge_extractions_before(results_cutoff, BATCH)
        })
        .await?;
        let outcomes =
            drain(|| repos.retention.purge_outcomes_before(results_cutoff, BATCH)).await?;
        Ok(facts + recalls + results + failures + extractions + outcomes)
    }

    async fn retire_expired(repos: &BrainRepositories) -> Result<u64, RepositoryError> {
        let started = Instant::now();
        let mut retired = 0;
        while started.elapsed() < BUDGET {
            let expired = repos.retention.expired(Utc::now(), BATCH).await?;
            let mut tx = repos.write_pool().begin().await?;
            for id in &expired {
                FactRepository::apply(&mut tx, id, FactChange::Expired).await?;
            }
            tx.commit().await?;
            retired += u64::try_from(expired.len()).unwrap_or(u64::MAX);
            if expired.len() < BATCH_LEN {
                break;
            }
        }
        Ok(retired)
    }
}

#[async_trait]
impl Job for BrainRetentionJob {
    fn name(&self) -> &'static str {
        "brain_retention"
    }

    fn description(&self) -> &'static str {
        "Deletes retired brain facts, old recalls, capture results and outcome signals past retention, and retires expired facts"
    }

    fn schedule(&self) -> &'static str {
        "0 */10 * * * *"
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let started = Instant::now();
        let profile = ctx.get::<Arc<BrainProfile>>()?;
        if !profile.enabled {
            return Ok(JobResult::success().with_stats(0, 0));
        }
        let repos = BrainRepositories::new(ctx.get::<DbPool>()?);
        let internal = |e: RepositoryError| ProviderError::Internal(Box::new(e));
        let purged = Self::purge(&repos, &profile.lifecycle)
            .await
            .map_err(internal)?;
        let retired = Self::retire_expired(&repos).await.map_err(internal)?;
        if purged + retired > 0 {
            tracing::info!(purged, retired, "Brain retention completed");
        }
        Ok(JobResult::success()
            .with_stats(purged + retired, 0)
            .with_duration(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)))
    }
}

submit_job!(&BrainRetentionJob);
