//! The consolidation job's inactive paths: with the profile off, with no
//! consolidator registered, or with no model client, it does nothing and
//! answers success with zero items; none of these touch a database.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_brain::jobs::{BrainConsolidateJob, ConsolidationInactive, ConsolidationParts};
use systemprompt_extension::brain::{
    BrainConsolidateError, BrainModelClient, BrainModelError, ConsolidationInput, FactConsolidator,
};
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::brain::{BrainModelRequest, BrainModelResponse, FactCandidate, KindDecl};
use systemprompt_traits::{Dependencies, Job, JobContext};

struct NoopConsolidator;

#[async_trait]
impl FactConsolidator for NoopConsolidator {
    fn name(&self) -> &'static str {
        "test.noop"
    }

    fn handles(&self, _kind: &KindDecl) -> bool {
        true
    }

    async fn consolidate(
        &self,
        _input: &ConsolidationInput,
    ) -> Result<Vec<FactCandidate>, BrainConsolidateError> {
        Ok(Vec::new())
    }
}

struct NoModel;

#[async_trait]
impl BrainModelClient for NoModel {
    async fn complete(
        &self,
        _request: BrainModelRequest,
    ) -> Result<BrainModelResponse, BrainModelError> {
        Err(BrainModelError::Unavailable)
    }
}

fn profile(enabled: bool, consolidation: bool) -> BrainProfile {
    let mut profile = BrainProfile {
        enabled,
        ..BrainProfile::default()
    };
    profile.consolidation.enabled = consolidation;
    profile
}

fn ctx(profile: BrainProfile) -> JobContext {
    JobContext::new(
        Actor::user(UserId::generate()),
        Dependencies::new().with(Arc::new(profile)),
    )
}

fn parts(consolidators: usize, model: bool) -> ConsolidationParts {
    ConsolidationParts {
        consolidators: (0..consolidators)
            .map(|_| Arc::new(NoopConsolidator) as Arc<dyn FactConsolidator>)
            .collect(),
        model: model.then(|| Arc::new(NoModel) as Arc<dyn BrainModelClient>),
        sink: None,
    }
}

#[test]
fn consolidation_is_off_by_default() {
    let profile = BrainProfile::default();
    assert!(!profile.consolidation.enabled);
    assert_eq!(profile.consolidation.daily_token_budget, None);
    assert_eq!(
        profile.consolidation.budget_tokens(&profile.extraction),
        profile.extraction.daily_token_budget
    );
}

#[test]
fn the_inactive_reason_is_the_first_the_operator_can_act_on() {
    let on = profile(true, true);
    assert_eq!(
        parts(1, true).inactive_reason(&profile(true, false)),
        Some(ConsolidationInactive::Disabled)
    );
    assert_eq!(
        parts(0, true).inactive_reason(&on),
        Some(ConsolidationInactive::NoConsolidator)
    );
    assert_eq!(
        parts(1, false).inactive_reason(&on),
        Some(ConsolidationInactive::NoModelClient)
    );
    assert_eq!(
        parts(1, true).inactive_reason(&on),
        Some(ConsolidationInactive::NoSink)
    );
}

#[tokio::test]
async fn disabled_consolidation_succeeds_with_nothing_done() {
    let result = BrainConsolidateJob::run_with(&ctx(profile(true, false)), parts(1, true))
        .await
        .expect("runs");
    assert!(result.success);
    assert_eq!(result.items_processed, Some(0));
}

#[tokio::test]
async fn no_registered_consolidator_succeeds_with_nothing_done() {
    let result = BrainConsolidateJob::run_with(&ctx(profile(true, true)), parts(0, true))
        .await
        .expect("runs");
    assert!(result.success);
    assert_eq!(result.items_processed, Some(0));
}

#[tokio::test]
async fn a_disabled_brain_never_consolidates() {
    let result = BrainConsolidateJob::run_with(&ctx(profile(false, true)), parts(1, true))
        .await
        .expect("runs");
    assert!(result.success);
    assert_eq!(result.items_processed, Some(0));
}

#[test]
fn the_job_runs_daily_after_the_analyses() {
    assert_eq!(BrainConsolidateJob.name(), "brain_consolidate");
    assert_eq!(BrainConsolidateJob.schedule(), "0 45 3 * * *");
    assert!(BrainConsolidateJob.description().contains("inactive"));
}
