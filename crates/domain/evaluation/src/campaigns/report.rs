//! Recommendations derive eligibility from retained execution measurements,
//! never from client-supplied scores or publication review prose.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use std::collections::BTreeMap;
use systemprompt_identifiers::{EvalCampaignId, EvalExperimentId, EvalRevisionId, UserId};

use super::comparison::{self, ComparisonDecision, Outcome, PairedOutcome};
use super::repository::CampaignRecord;
use crate::Result;
use crate::experiments::conflict;
use crate::experiments::records::{ExecutionStatus, ExperimentRecord, ExperimentStatus};
use crate::experiments::resources::{Partition, ResourceContent};
use crate::models::AccountingStatus;
use crate::repository::experiments::{
    EvaluationRepositories, MeasurementRow, RetainedMeasurement, RevisionRepository,
};

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct CampaignReport {
    pub execution_availability: crate::repository::experiments::CampaignAvailability,
    pub diagnostics: Vec<super::diagnostics::CampaignDiagnostic>,
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
    let rows = report.variants;
    let retained = pair_measurements(
        rows,
        &experiment.experiment.spec.cases,
        experiment.experiment.spec.repetitions,
    )?;
    let mut limitations = retained.limitations;
    let mut development = Vec::new();
    let mut holdout = Vec::new();
    for (case, repetitions) in retained.cases {
        let ResourceContent::Case(case) = revisions.get(owner, &case).await? else {
            return Err(conflict("Invalid case revision"));
        };
        let pair = comparison::collapse_repetitions(&repetitions)?;
        match case.partition {
            Partition::Development => development.push(pair),
            Partition::Holdout => holdout.push(pair),
        }
    }
    limitations.extend(experiment_limitations(&experiment.experiment));
    let development = comparison::compare(&campaign.policy, &development)?;
    let holdout = comparison::compare(&campaign.policy, &holdout)?;
    Ok(CampaignReport {
        execution_availability: repositories
            .experiments
            .execution_availability(&experiment.experiment.spec),
        diagnostics: repositories
            .campaigns
            .diagnostics(owner, Some(campaign_id), None, 100)
            .await?,
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

fn experiment_limitations(experiment: &ExperimentRecord) -> Vec<String> {
    let mut limitations = Vec::new();
    if experiment.status != ExperimentStatus::Completed {
        limitations.push("Experiment is not completed".to_owned());
    }
    if !experiment.spec.claim_independent_improvement {
        limitations.push("A fresh independent holdout was not reserved".to_owned());
    }
    if experiment.accounting.reserved != 0 || experiment.accounting.frozen {
        limitations.push("Accounting has unsettled reservations or is frozen".to_owned());
    }
    limitations
}

struct RetainedPairs {
    cases: BTreeMap<EvalRevisionId, Vec<PairedOutcome>>,
    limitations: Vec<String>,
}

fn pair_measurements(
    rows: Vec<MeasurementRow>,
    expected_cases: &[EvalRevisionId],
    repetitions: u32,
) -> Result<RetainedPairs> {
    if expected_cases.is_empty() || expected_cases.len() > 100 || !(1..=10).contains(&repetitions) {
        return Err(conflict("Invalid frozen execution matrix bounds"));
    }
    let repetitions = i32::try_from(repetitions)
        .map_err(|error| conflict(&format!("Invalid repetition count: {error}")))?;
    let mut pairs: BTreeMap<(EvalRevisionId, i32), [Option<Outcome>; 2]> = expected_cases
        .iter()
        .flat_map(|case| {
            (0..repetitions).map(move |repetition| ((case.clone(), repetition), [None, None]))
        })
        .collect();
    let mut observed = std::collections::BTreeSet::new();
    let mut limitations = Vec::new();
    for row in rows {
        if row.variant > 1 {
            return Err(conflict("Unexpected experiment variant"));
        }
        if !observed.insert((row.case_revision_id.clone(), row.repetition, row.variant)) {
            return Err(conflict("Duplicate paired measurement"));
        }
        let pair = pairs
            .get_mut(&(row.case_revision_id, row.repetition))
            .ok_or_else(|| conflict("Measurement is outside the frozen execution matrix"))?;
        if let Some(measurement) = row
            .measurement
            .filter(|_| row.status == ExecutionStatus::Completed)
        {
            pair[row.variant] = outcome(&measurement);
        }
        if pair[row.variant].is_none() {
            limitations.push(format!(
                "Execution {} has incomplete outcome evidence",
                row.execution_id
            ));
        }
    }
    let mut cases: BTreeMap<EvalRevisionId, Vec<PairedOutcome>> = BTreeMap::new();
    for ((case, repetition), pair) in pairs {
        if let [Some(baseline), Some(candidate)] = pair {
            cases.entry(case).or_default().push(PairedOutcome {
                baseline,
                candidate,
            });
        } else {
            limitations.push(format!(
                "Case {case}, repetition {repetition} has an incomplete baseline/candidate pair"
            ));
        }
    }
    Ok(RetainedPairs { cases, limitations })
}

fn outcome(measurement: &RetainedMeasurement) -> Option<Outcome> {
    Some(Outcome {
        quality_milli: measurement.quality_milli?,
        tokens: measurement
            .input_tokens?
            .checked_add(measurement.output_tokens?)?,
        cost_microdollars: u64::try_from(measurement.attempted_cost_microdollars).ok()?,
        latency_ms: measurement.latency_ms?,
        verified_success: measurement.verified_success,
        hard_failures: u32::try_from(measurement.hard_failures.len()).ok()?,
        accounting_complete: measurement.accounting_status == AccountingStatus::Complete,
    })
}
