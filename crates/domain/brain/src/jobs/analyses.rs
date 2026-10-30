//! [`BrainAnalysisJob`]: once a day, every core analysis and then every
//! extension analysis reads the last day of history, and each draft that
//! no open proposal already carries is stored as an open proposal.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionContext;
use systemprompt_extension::brain::{AnalysisWindow, BrainAnalysis, discover_brain_analyses};
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_provider_contracts::{JobScope, submit_job};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderError, ProviderResult};

use crate::analyses::{
    AnalysisRepository, BloatAnalysis, BloatThresholds, CoverageAnalysis, CoverageThresholds,
    RelevanceAnalysis, RelevanceThresholds, fresh_drafts,
};
use crate::repository::{BrainRepositories, ProposalRepository};

const OPEN_SCAN: i64 = 1000;

/// Runs the brain analyses and stores their new proposals.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrainAnalysisJob;

fn analyses(repos: &BrainRepositories, ctx: &JobContext) -> Vec<Arc<dyn BrainAnalysis>> {
    let store = AnalysisRepository::new(Arc::new(repos.read_pool().clone()));
    let mut all: Vec<Arc<dyn BrainAnalysis>> = vec![
        Arc::new(BloatAnalysis::new(
            store.clone(),
            BloatThresholds::default(),
        )),
        Arc::new(RelevanceAnalysis::new(
            store.clone(),
            RelevanceThresholds::default(),
        )),
        Arc::new(CoverageAnalysis::new(store, CoverageThresholds::default())),
    ];
    if let Ok(extensions) = ctx.get::<Arc<dyn ExtensionContext>>() {
        all.extend(
            discover_brain_analyses()
                .iter()
                .map(|r| (r.factory)(extensions.as_ref())),
        );
    }
    all
}

async fn store_fresh(
    repos: &BrainRepositories,
    analysis: &dyn BrainAnalysis,
    window: AnalysisWindow,
) -> Result<u64, ProviderError> {
    let drafts = analysis
        .analyse(window)
        .await
        .map_err(|e| ProviderError::Internal(Box::new(e)))?;
    let open = repos
        .proposals
        .list_open(OPEN_SCAN)
        .await
        .map_err(|e| ProviderError::Internal(Box::new(e)))?;
    let mut stored = 0;
    for draft in fresh_drafts(drafts, &open) {
        ProposalRepository::insert(repos.write_pool(), &draft)
            .await
            .map_err(|e| ProviderError::Internal(Box::new(e)))?;
        stored += 1;
    }
    Ok(stored)
}

#[async_trait]
impl Job for BrainAnalysisJob {
    fn name(&self) -> &'static str {
        "brain_analysis"
    }

    fn description(&self) -> &'static str {
        "Drafts brain proposals from measured history: bloated kinds, citation-weighted importance and coverage gaps"
    }

    fn schedule(&self) -> &'static str {
        "0 15 3 * * *"
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let started = Instant::now();
        if !ctx.get::<Arc<BrainProfile>>()?.enabled {
            return Ok(JobResult::success().with_stats(0, 0));
        }
        let repos = BrainRepositories::new(ctx.get::<DbPool>()?);
        let until = Utc::now();
        let window = AnalysisWindow {
            since: until - Duration::days(1),
            until,
        };
        let (mut stored, mut failed) = (0u64, 0u64);
        for analysis in analyses(&repos, ctx) {
            match store_fresh(&repos, analysis.as_ref(), window).await {
                Ok(count) => stored += count,
                Err(error) => {
                    failed += 1;
                    tracing::warn!(analysis = analysis.name(), error = %error, "Brain analysis failed");
                },
            }
        }
        let duration = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let result = if failed == 0 {
            JobResult::success()
        } else {
            JobResult::failure(format!("{failed} brain analyses failed"))
        };
        Ok(result.with_stats(stored, failed).with_duration(duration))
    }
}

submit_job!(&BrainAnalysisJob);
