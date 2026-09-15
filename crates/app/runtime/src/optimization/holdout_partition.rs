//! Case partitioning and the frozen holdout specification for an optimisation
//! campaign: development cases and the holdout dataset must not overlap, and
//! the frozen spec carries the dataset digest its budget was computed from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{OptimizationError, SkillOptimizationOrchestrator};
use systemprompt_evaluation::experiments::resources::{CaseContent, Partition, ResourceContent};
use systemprompt_evaluation::experiments::{ExperimentSpec, content_digest};
use systemprompt_identifiers::{EvalRevisionId, UserId};

pub(super) struct PartitionedCases {
    pub(super) development: Vec<EvalRevisionId>,
    pub(super) holdout: Vec<EvalRevisionId>,
}

fn case_digest(case: &CaseContent) -> Result<String, OptimizationError> {
    Ok(content_digest(&(
        &case.prompt,
        &case.expected_behavior,
        &case.fixtures,
        &case.assertions,
    ))?)
}

impl SkillOptimizationOrchestrator {
    pub(super) async fn partition_cases(
        &self,
        owner: &UserId,
        development_cases: &[EvalRevisionId],
        holdout_dataset: &EvalRevisionId,
    ) -> Result<PartitionedCases, OptimizationError> {
        let mut development = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for id in development_cases {
            if let ResourceContent::Case(case) = self.evaluations.revisions.get(owner, id).await?
                && case.partition == Partition::Development
            {
                seen.insert(case_digest(&case)?);
                development.push(id.clone());
            }
        }
        let ResourceContent::Dataset(ids) = self
            .evaluations
            .revisions
            .get(owner, holdout_dataset)
            .await?
        else {
            return Err(OptimizationError::Source(
                "Select a retained holdout dataset".to_owned(),
            ));
        };
        let mut holdout = Vec::new();
        for id in ids {
            let ResourceContent::Case(case) = self.evaluations.revisions.get(owner, &id).await?
            else {
                return Err(OptimizationError::Source(
                    "Holdout dataset must contain cases".to_owned(),
                ));
            };
            if case.partition != Partition::Holdout {
                continue;
            }
            if !seen.insert(case_digest(&case)?) {
                return Err(OptimizationError::Source(
                    "Holdout must not duplicate development or another holdout case".to_owned(),
                ));
            }
            holdout.push(id);
        }
        Ok(PartitionedCases {
            development,
            holdout,
        })
    }
    pub(super) async fn freeze_holdout_spec(
        &self,
        owner: &UserId,
        spec: &mut ExperimentSpec,
        cases: PartitionedCases,
    ) -> Result<(), OptimizationError> {
        let PartitionedCases {
            mut development,
            holdout,
        } = cases;
        development.extend(holdout);
        spec.cases = development;
        let dataset = ResourceContent::Dataset(spec.cases.clone());
        let dataset_id = self
            .evaluations
            .revisions
            .create(
                owner,
                &format!("holdout:{}", content_digest(&dataset)?),
                &dataset,
            )
            .await?;
        spec.dataset = Some(dataset_id);
        spec.claim_independent_improvement = true;
        let frozen = spec.frozen.as_mut().ok_or_else(|| {
            OptimizationError::Source("Holdout requires frozen execution settings".to_owned())
        })?;
        frozen.dataset_digest = content_digest(&dataset)?;
        let executions = u64::try_from(spec.cases.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(2)
            .saturating_mul(u64::from(spec.repetitions));
        spec.budget_microdollars = frozen.cost_envelope.maximum_microdollars(executions)?;
        spec.validate()?;
        Ok(())
    }
}
