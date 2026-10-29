//! [`NotificationRetentionJob`]: every five minutes, three bounded purges,
//! each with its own time budget so one cannot starve the others:
//!
//! - read notifications older than `notifications.retention_days`, and every
//!   row of a stream older than that stream's own `retention_days`;
//! - delivery failures older than `failure_retention_days`;
//! - unread notifications older than `unread_max_age_days`, deleted rather than
//!   marked read: a row nobody read in that time is no longer actionable, and
//!   marking it read would only keep it for another retention period.
//!
//! Deletes run in batches; whatever is left waits for the next run. The
//! profile block reaches the job as an `Arc<NotificationsProfile>` in the job
//! context's dependencies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_manifest::profile::NotificationsProfile;
use systemprompt_provider_contracts::{JobScope, ProviderError, submit_job};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult, RepositoryError};

use crate::repository::NotificationRepositories;

const BATCH: i64 = 1000;
const BUDGET: Duration = Duration::from_secs(10);

/// Purges read, past-retention, stale unread and old failure rows.
#[derive(Debug, Clone, Copy, Default)]
pub struct NotificationRetentionJob;

async fn drain<F, Fut>(started: Instant, mut step: F) -> Result<u64, RepositoryError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<u64, RepositoryError>>,
{
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

impl NotificationRetentionJob {
    async fn purge_read(
        repos: &NotificationRepositories,
        profile: &NotificationsProfile,
    ) -> Result<u64, RepositoryError> {
        let started = Instant::now();
        let read_cutoff = cutoff(profile.retention_days);
        let mut purged = drain(started, || {
            repos.notifications.purge_read_before(read_cutoff, BATCH)
        })
        .await?;
        for (stream, days) in repos.streams.list_retention().await? {
            let stream_cutoff = cutoff(days);
            purged += drain(started, || {
                repos
                    .notifications
                    .purge_stream_retention(&stream, stream_cutoff, BATCH)
            })
            .await?;
        }
        Ok(purged)
    }

    async fn purge(
        repos: &NotificationRepositories,
        profile: &NotificationsProfile,
    ) -> Result<u64, RepositoryError> {
        let read = Self::purge_read(repos, profile).await?;
        let failure_cutoff = cutoff(profile.failure_retention_days);
        let failures = drain(Instant::now(), || {
            repos.failures.purge_before(failure_cutoff, BATCH)
        })
        .await?;
        let unread_cutoff = cutoff(profile.unread_max_age_days);
        let unread = drain(Instant::now(), || {
            repos
                .notifications
                .purge_unread_before(unread_cutoff, BATCH)
        })
        .await?;
        Ok(read + failures + unread)
    }
}

fn cutoff(days: u32) -> DateTime<Utc> {
    Utc::now() - chrono::Duration::days(i64::from(days))
}

#[async_trait]
impl Job for NotificationRetentionJob {
    fn name(&self) -> &'static str {
        "notification_retention"
    }

    fn description(&self) -> &'static str {
        "Purges read, past-retention and stale unread notifications and old delivery failures"
    }

    fn schedule(&self) -> &'static str {
        "0 */5 * * * *"
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let started = Instant::now();
        let profile = ctx.get::<Arc<NotificationsProfile>>()?;
        let repos = NotificationRepositories::new(ctx.get::<DbPool>()?);
        let purged = Self::purge(&repos, &profile)
            .await
            .map_err(|e| ProviderError::Internal(Box::new(e)))?;
        if purged > 0 {
            tracing::info!(purged, "Notification retention purge completed");
        }
        Ok(JobResult::success()
            .with_stats(purged, 0)
            .with_duration(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)))
    }
}

submit_job!(&NotificationRetentionJob);
