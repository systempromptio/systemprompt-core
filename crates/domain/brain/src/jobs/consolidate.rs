//! [`BrainConsolidateJob`]: scaffolding, inactive unless an extension opts
//! in. Once a day it offers each declared kind's facts, grouped by subject,
//! to the first registered [`FactConsolidator`] that handles the kind, and
//! enters every candidate it answers as a `proposed` fact.
//!
//! The job does nothing, and says so once per process, unless all of these
//! hold: `brain.consolidation.enabled` is `true`, at least one consolidator
//! is registered, the job context carries a brain model client and the
//! brain sink. Core registers no consolidator and makes no model call
//! here; the consolidator decides whether to call the model it is handed,
//! under the day's remaining token budget.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use chrono::Utc;
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionContext;
use systemprompt_extension::brain::{
    BrainModelClient, DynBrainSink, FactConsolidator, discover_brain_consolidators,
};
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_provider_contracts::{JobScope, submit_job};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderError, ProviderResult};

use super::consolidate_run::{ConsolidationRun, RunTotals};
use crate::pipeline::metrics;
use crate::repository::{BrainRepositories, ExtractionRepository};

/// Enters consolidated facts proposed by registered consolidators.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrainConsolidateJob;

/// Why a run did nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsolidationInactive {
    Disabled,
    NoConsolidator,
    NoModelClient,
    NoSink,
}

impl fmt::Display for ConsolidationInactive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Disabled => "disabled",
            Self::NoConsolidator => "no consolidator registered",
            Self::NoModelClient => "no brain model client",
            Self::NoSink => "no brain sink",
        })
    }
}

fn inactive(reason: ConsolidationInactive) -> JobResult {
    tracing::info!(reason = %reason, "brain consolidation inactive");
    metrics::consolidation("inactive");
    JobResult::success().with_stats(0, 0)
}

/// What a run needs beyond the profile and the pool.
pub struct ConsolidationParts {
    pub consolidators: Vec<Arc<dyn FactConsolidator>>,
    pub model: Option<Arc<dyn BrainModelClient>>,
    pub sink: Option<DynBrainSink>,
}

impl fmt::Debug for ConsolidationParts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConsolidationParts")
            .field("consolidators", &self.consolidators.len())
            .field("model", &self.model.is_some())
            .field("sink", &self.sink.is_some())
            .finish()
    }
}

impl ConsolidationParts {
    fn from_context(ctx: &JobContext) -> Self {
        let consolidators = ctx
            .get::<Arc<dyn ExtensionContext>>()
            .map(|extensions| {
                discover_brain_consolidators()
                    .iter()
                    .map(|r| (r.factory)(extensions.as_ref()))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            consolidators,
            model: ctx
                .get::<Option<Arc<dyn BrainModelClient>>>()
                .ok()
                .cloned()
                .flatten(),
            sink: ctx.get::<DynBrainSink>().ok().cloned(),
        }
    }

    #[must_use]
    pub fn inactive_reason(&self, profile: &BrainProfile) -> Option<ConsolidationInactive> {
        if !profile.consolidation.enabled {
            return Some(ConsolidationInactive::Disabled);
        }
        if self.consolidators.is_empty() {
            return Some(ConsolidationInactive::NoConsolidator);
        }
        if self.model.is_none() {
            return Some(ConsolidationInactive::NoModelClient);
        }
        if self.sink.is_none() {
            return Some(ConsolidationInactive::NoSink);
        }
        None
    }
}

impl BrainConsolidateJob {
    pub async fn run_with(
        ctx: &JobContext,
        parts: ConsolidationParts,
    ) -> ProviderResult<JobResult> {
        let started = Instant::now();
        let profile = ctx.get::<Arc<BrainProfile>>()?;
        if !profile.enabled {
            return Ok(JobResult::success().with_stats(0, 0));
        }
        if let Some(reason) = parts.inactive_reason(profile) {
            return Ok(inactive(reason));
        }
        let (Some(model), Some(sink)) = (parts.model, parts.sink) else {
            return Ok(inactive(ConsolidationInactive::NoModelClient));
        };
        let repos = BrainRepositories::new(ctx.get::<DbPool>()?);
        let remaining = remaining_budget(&repos, profile)
            .await
            .map_err(|e| ProviderError::Internal(Box::new(e)))?;
        if remaining == 0 {
            metrics::budget_exhausted();
            metrics::consolidation("budget_exhausted");
            return Ok(JobResult::success().with_stats(0, 0));
        }
        let run = ConsolidationRun {
            repos: &repos,
            consolidators: &parts.consolidators,
            model,
            sink,
            actor: ctx.actor().user_id.clone(),
            budget_tokens: remaining,
        };
        let RunTotals { entered, failed } = run
            .execute()
            .await
            .map_err(|e| ProviderError::Internal(Box::new(e)))?;
        let result = if failed == 0 {
            JobResult::success()
        } else {
            JobResult::failure(format!("{failed} brain consolidations failed"))
        };
        Ok(result
            .with_stats(entered, failed)
            .with_duration(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)))
    }
}

async fn remaining_budget(
    repos: &BrainRepositories,
    profile: &BrainProfile,
) -> Result<u64, systemprompt_traits::RepositoryError> {
    let now = Utc::now();
    let day_start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map_or(now, |midnight| midnight.and_utc());
    let used = ExtractionRepository::tokens_since(repos.read_pool(), day_start).await?;
    let budget = profile.consolidation.budget_tokens(&profile.extraction);
    Ok(budget.saturating_sub(u64::try_from(used).unwrap_or(0)))
}

#[async_trait]
impl Job for BrainConsolidateJob {
    fn name(&self) -> &'static str {
        "brain_consolidate"
    }

    fn description(&self) -> &'static str {
        "Offers each kind's facts to registered consolidators and enters their answers as proposed facts; inactive until an extension opts in"
    }

    fn schedule(&self) -> &'static str {
        "0 45 3 * * *"
    }

    fn scope(&self) -> JobScope {
        JobScope::Cluster
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        Self::run_with(ctx, ConsolidationParts::from_context(ctx)).await
    }
}

submit_job!(&BrainConsolidateJob);
