//! Recommendations derive eligibility from retained execution measurements,
//! never from client-supplied scores or publication review prose.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use systemprompt_identifiers::{
    EvalCampaignId, EvalExecutionId, EvalExperimentId, EvalRevisionId, UserId,
};

use super::comparison::{self, ComparisonDecision, Outcome, PairedOutcome};
use super::repository::CampaignRecord;
use crate::Result;
use crate::experiments::conflict;
use crate::experiments::records::ExperimentStatus;
use crate::experiments::resources::{Partition, ResourceContent};
use crate::repository::experiments::{
    DeterministicMeasurement, EvaluationRepositories, RevisionRepository,
};

#[derive(Debug, Clone, Serialize)]
pub struct CampaignReport {
    pub suggestions: Vec<super::suggestions::RetainedSuggestion>,
    pub campaign: CampaignRecord,
    pub experiment_id: EvalExperimentId,
    pub baseline_bundle_digest: String,
    pub candidate_bundle_digest: String,
    pub development: ComparisonDecision,
    pub holdout: ComparisonDecision,
    pub eligible_for_publication: bool,
    pub limitations: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct MeasurementRow {
    execution_id: EvalExecutionId,
    variant: usize,
    case_revision_id: EvalRevisionId,
    repetition: i32,
    status: String,
    measurement: Option<DeterministicMeasurement>,
}

pub async fn build(
    repositories: &EvaluationRepositories,
    revisions: &RevisionRepository,
    owner: &UserId,
    campaign_id: &EvalCampaignId,
    experiment_id: &EvalExperimentId,
) -> Result<CampaignReport> {
    let campaign = repositories.campaigns.get(owner, campaign_id).await?;
    let ids = repositories
        .campaigns
        .list_experiments(owner, campaign_id)
        .await?;
    if !ids.contains(experiment_id) {
        return Err(conflict("Experiment is not part of this campaign"));
    }
    let experiment = repositories.experiments.get(owner, experiment_id).await?;
    if experiment.experiment.spec.variants.len() != 2 {
        return Err(conflict(
            "Campaign reports require exactly two paired variants",
        ));
    }
    let report = repositories
        .lifecycle
        .comparison(owner, experiment_id)
        .await?;
    let rows: Vec<MeasurementRow> = serde_json::from_value(report.variants)?;
    let mut pairs: BTreeMap<(EvalRevisionId, i32), [Option<Outcome>; 2]> = BTreeMap::new();
    let mut limitations = Vec::new();
    for row in rows {
        if row.variant > 1 {
            return Err(conflict("Unexpected experiment variant"));
        }
        let pair = pairs
            .entry((row.case_revision_id, row.repetition))
            .or_insert([None, None]);
        if pair[row.variant].is_some() {
            return Err(conflict("Duplicate paired measurement"));
        }
        if let Some(measurement) = row.measurement.filter(|_| row.status == "completed") {
            pair[row.variant] = outcome(&measurement);
        }
        if pair[row.variant].is_none() {
            limitations.push(format!(
                "Execution {} has incomplete outcome evidence",
                row.execution_id
            ));
        }
    }
    let mut development = Vec::new();
    let mut holdout = Vec::new();
    let mut cases: BTreeMap<EvalRevisionId, Vec<PairedOutcome>> = BTreeMap::new();
    for ((case, _), pair) in pairs {
        if let [Some(baseline), Some(candidate)] = pair {
            cases.entry(case).or_default().push(PairedOutcome {
                baseline,
                candidate,
            });
        }
    }
    for (case, repetitions) in cases {
        let ResourceContent::Case(case) = revisions.get(owner, &case).await? else {
            return Err(conflict("Invalid case revision"));
        };
        let pair = comparison::collapse_repetitions(&repetitions)?;
        match case.partition {
            Partition::Development => development.push(pair),
            Partition::Holdout => holdout.push(pair),
        }
    }
    if experiment.experiment.status != ExperimentStatus::Completed {
        limitations.push("Experiment is not completed".to_owned());
    }
    if !experiment.experiment.spec.claim_independent_improvement {
        limitations.push("A fresh independent holdout was not reserved".to_owned());
    }
    if experiment.experiment.accounting.reserved != 0 || experiment.experiment.accounting.frozen {
        limitations.push("Accounting has unsettled reservations or is frozen".to_owned());
    }
    let development = comparison::compare(&campaign.policy, &development)?;
    let holdout = comparison::compare(&campaign.policy, &holdout)?;
    Ok(CampaignReport {
        suggestions: repositories
            .lifecycle
            .list_suggestions(owner, experiment_id)
            .await?,
        campaign,
        experiment_id: experiment_id.clone(),
        baseline_bundle_digest: experiment.experiment.spec.variants[0]
            .skill_bundle_digest
            .clone(),
        candidate_bundle_digest: experiment.experiment.spec.variants[1]
            .skill_bundle_digest
            .clone(),
        eligible_for_publication: limitations.is_empty()
            && development.eligible
            && holdout.eligible,
        development,
        holdout,
        limitations,
    })
}

fn outcome(measurement: &DeterministicMeasurement) -> Option<Outcome> {
    Some(Outcome {
        quality_milli: measurement.quality_milli?,
        tokens: measurement
            .input_tokens?
            .checked_add(measurement.output_tokens?)?,
        cost_microdollars: u64::try_from(measurement.attempted_cost_microdollars).ok()?,
        latency_ms: measurement.latency_ms,
        verified_success: measurement.verified_success,
        hard_failures: u32::try_from(measurement.hard_failures.len()).ok()?,
        accounting_complete: measurement.accounting_status == "complete",
    })
}
