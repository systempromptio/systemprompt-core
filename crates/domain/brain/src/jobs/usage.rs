//! [`BrainUsageJob`]: once a day, before retention and the analyses, every
//! fact's injections and citations over the results window become two
//! Laplace-smoothed rates on the fact, which recall's in-context score
//! reads. Deterministic, no model call.
//!
//! `injected` counts the recall rows that were not skipped and spent
//! tokens (a same-turn reuse is not a second injection), `cited` the
//! `cited_in_response` outcomes on those recalls, and `ignored` the
//! difference. With the prior `(x + 1) / (n + 2)` a fact never injected
//! sits at `0.5` on both rates, so usage orders only facts that were
//! actually shown.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use systemprompt_database::DbPool;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_provider_contracts::{JobScope, submit_job};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderError, ProviderResult};

use crate::pipeline::metrics;
use crate::repository::{BrainRepositories, FactUsageRates, UsageCounts};

const WRITE_BATCH: usize = 1000;

/// Writes each fact's usage rates from the results window.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrainUsageJob;

/// The smoothed rates behind one fact's counts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UsageRates {
    pub cited_rate: f64,
    pub ignored_rate: f64,
}

#[must_use]
pub fn laplace_rates(injected: u64, cited: u64) -> UsageRates {
    let cited = cited.min(injected);
    let ignored = injected - cited;
    let denominator = count_to_f64(injected) + 2.0;
    UsageRates {
        cited_rate: (count_to_f64(cited) + 1.0) / denominator,
        ignored_rate: (count_to_f64(ignored) + 1.0) / denominator,
    }
}

fn count_to_f64(count: u64) -> f64 {
    u32::try_from(count).map_or_else(|_| f64::from(u32::MAX), f64::from)
}

fn rates_of(counts: &UsageCounts) -> FactUsageRates {
    let rates = laplace_rates(counts.injected, counts.cited);
    FactUsageRates {
        fact_id: counts.fact_id.clone(),
        cited_rate: rates.cited_rate,
        ignored_rate: rates.ignored_rate,
    }
}

#[async_trait]
impl Job for BrainUsageJob {
    fn name(&self) -> &'static str {
        "brain_usage"
    }

    fn description(&self) -> &'static str {
        "Sets each brain fact's cited and ignored rates from the recalls and citations of the results window"
    }

    fn schedule(&self) -> &'static str {
        "0 45 2 * * *"
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
        let internal = |e| ProviderError::Internal(Box::new(e));
        let since =
            Utc::now() - Duration::days(i64::from(profile.lifecycle.results_retention_days));
        let counts = repos
            .usage
            .list_counts_since(since)
            .await
            .map_err(internal)?;
        let rates: Vec<FactUsageRates> = counts.iter().map(rates_of).collect();
        let mut updated = 0u64;
        for batch in rates.chunks(WRITE_BATCH) {
            updated += repos
                .usage
                .set_rates_detached(batch)
                .await
                .map_err(internal)?;
        }
        metrics::usage_updated(updated);
        tracing::info!(updated, facts = counts.len(), "Brain usage rates written");
        Ok(JobResult::success()
            .with_stats(updated, 0)
            .with_duration(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)))
    }
}

submit_job!(&BrainUsageJob);
