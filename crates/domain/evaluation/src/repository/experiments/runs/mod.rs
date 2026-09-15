//! Owner-scoped experiment creation, cancellation and durable worker claiming.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use crate::experiments::records::{
    ExecutionRecord, ExperimentDetail, ExperimentPreflight, ExperimentRecord,
};
use crate::experiments::resources::ResourceContent;
use sqlx::PgPool;
use sqlx::types::Json;
use systemprompt_identifiers::{
    EvalBudgetId, EvalExecutionId, EvalExperimentId, EvalWorkerId, UserId,
};

use super::{BudgetRepository, RevisionRepository};
use crate::experiments::{ExperimentSpec, content_digest, invalid};

mod claims;

#[derive(Clone, Debug)]
pub struct ExperimentRepository {
    pub(super) pool: PgPool,
    revisions: RevisionRepository,
    budgets: BudgetRepository,
    pub(super) admission: std::sync::Arc<dyn crate::capabilities::ExecutionAdmission>,
}

impl ExperimentRepository {
    pub fn new(pool: PgPool, budgets: BudgetRepository) -> Self {
        Self::with_admission(
            pool,
            budgets,
            std::sync::Arc::new(crate::capabilities::VerifiedExecutionAdmission),
        )
    }

    pub fn with_admission(
        pool: PgPool,
        budgets: BudgetRepository,
        admission: std::sync::Arc<dyn crate::capabilities::ExecutionAdmission>,
    ) -> Self {
        Self {
            admission,
            revisions: RevisionRepository::new(pool.clone()),
            budgets,
            pool,
        }
    }

    pub fn execution_availability(&self, spec: &ExperimentSpec) -> super::CampaignAvailability {
        let result = self.admission.admit(spec);
        super::CampaignAvailability {
            platform: std::env::consts::OS.to_owned(),
            architecture: std::env::consts::ARCH.to_owned(),
            admitted: result.is_ok(),
            reason: result.err().map(|_| "Exact client/platform/version has not passed execution admission; inspect evaluator capabilities.".to_owned()),
            variants: spec.variants.clone(),
        }
    }

    pub async fn create_with_budget(
        &self,
        owner: &UserId,
        key: &str,
        budget: &EvalBudgetId,
        spec: &ExperimentSpec,
    ) -> Result<EvalExperimentId> {
        let preflight = self.preflight(owner, budget, spec).await?;
        if !preflight.affordable {
            return Err(crate::EvaluationError::budget_exhausted(&preflight));
        }
        if key.trim().is_empty() || key.len() > 255 {
            return Err(invalid("An idempotency key is required"));
        }
        if !matches!(
            self.revisions.get(owner, &spec.rubric).await?,
            ResourceContent::Rubric(_)
        ) {
            return Err(invalid("Rubric reference must identify a rubric revision"));
        }
        for case in &spec.cases {
            if !matches!(
                self.revisions.get(owner, case).await?,
                ResourceContent::Case(_)
            ) {
                return Err(invalid("Case reference must identify a case revision"));
            }
        }
        let digest = content_digest(spec)?;
        let mut tx = self.pool.begin().await?;
        super::lock_owner(&mut tx, owner).await?;
        sqlx::query!(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            format!("eval:{}:{key}", owner.as_str())
        )
        .execute(&mut *tx)
        .await?;
        let existing = sqlx::query!(
            "SELECT id,spec_digest,budget_id FROM eval_experiments WHERE owner_id=$1 AND idempotency_key=$2",
            owner.as_str(), key
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing) = existing {
            if existing.spec_digest != digest || existing.budget_id != budget.as_str() {
                return Err(crate::experiments::conflict(
                    "Idempotency key conflicts with another experiment",
                ));
            }
            return Ok(EvalExperimentId::new(existing.id));
        }
        let account = sqlx::query!(
            "SELECT id FROM eval_budget_accounts WHERE id=$1 AND owner_id=$2 AND NOT frozen",
            budget.as_str(),
            owner.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?;
        if account.is_none() {
            return Err(crate::experiments::missing(
                "Active budget unavailable in this scope",
            ));
        }
        let id = EvalExperimentId::generate();
        sqlx::query!("INSERT INTO eval_experiments(id,owner_id,spec,spec_digest,budget_id,idempotency_key) VALUES($1,$2,$3,$4,$5,$6)", id.as_str(), owner.as_str(), Json(spec) as _, digest, budget.as_str(), key)
            .execute(&mut *tx).await?;
        Self::insert_executions(&mut tx, &id, spec).await?;
        super::holdout::consume(&mut tx, owner, &id, spec).await?;
        if spec.claim_independent_improvement {
            sqlx::query!("INSERT INTO eval_holdout_consumption(owner_id,case_revision_id,experiment_id) SELECT $1,c.id,$2 FROM eval_resource_revisions c WHERE c.owner_id=$1 AND c.id=ANY($3) AND c.content->'content'->>'partition'='holdout'",
                owner.as_str(), id.as_str(), &spec.cases.iter().map(|value| value.as_str().to_owned()).collect::<Vec<_>>()).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    pub async fn preflight(
        &self,
        owner: &UserId,
        budget: &EvalBudgetId,
        spec: &ExperimentSpec,
    ) -> Result<ExperimentPreflight> {
        self.admission.admit(spec)?;
        let dataset = spec
            .dataset
            .as_ref()
            .ok_or_else(|| invalid("Preflight requires a dataset revision"))?;
        let frozen = spec
            .frozen
            .as_ref()
            .ok_or_else(|| invalid("Preflight requires frozen environment settings"))?;
        let ResourceContent::Dataset(dataset_cases) = self.revisions.get(owner, dataset).await?
        else {
            return Err(invalid(
                "Dataset reference must identify a dataset revision",
            ));
        };
        if spec.cases.iter().any(|case| !dataset_cases.contains(case)) {
            return Err(invalid("Selected cases must belong to the frozen dataset"));
        }
        let rubric = self.revisions.get(owner, &spec.rubric).await?;
        if content_digest(&rubric)? != frozen.rubric_digest
            || content_digest(&ResourceContent::Dataset(dataset_cases))? != frozen.dataset_digest
        {
            return Err(invalid(
                "Dataset or rubric digest differs from frozen settings",
            ));
        }
        let (baseline, candidate) = crate::capabilities::paired_variants(spec)?;
        self.require_projections(
            owner,
            [
                &baseline.skill_bundle_digest,
                &candidate.skill_bundle_digest,
                &baseline.configuration_digest,
            ],
        )
        .await?;
        let account = self.budgets.get(owner, budget).await?;
        if spec.claim_independent_improvement {
            self.require_fresh_holdout(owner, spec).await?;
        }
        let execution_count = u64::try_from(spec.cases.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(2)
            .saturating_mul(u64::from(spec.repetitions));
        let maximum_cost_microdollars =
            frozen.cost_envelope.maximum_microdollars(execution_count)?;
        if spec.budget_microdollars != maximum_cost_microdollars {
            return Err(invalid(
                "Experiment budget must equal the conservatively derived frozen cost envelope",
            ));
        }
        let available = account
            .cap
            .saturating_sub(account.reserved)
            .saturating_sub(account.settled);
        Ok(ExperimentPreflight {
            execution_count,
            maximum_cost_microdollars,
            available_microdollars: available,
            affordable: !account.frozen && maximum_cost_microdollars <= available,
            matrix_digest: content_digest(spec)?,
        })
    }

    async fn require_projections(&self, owner: &UserId, digests: [&String; 3]) -> Result<()> {
        for digest in digests {
            let found = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM eval_managed_workspace_projections WHERE owner_id=$1 AND digest=$2)", owner.as_str(), digest).fetch_one(&self.pool).await?.unwrap_or(false);
            if !found {
                return Err(crate::experiments::missing(
                    "Managed workspace projection is unavailable",
                ));
            }
        }
        Ok(())
    }

    async fn require_fresh_holdout(&self, owner: &UserId, spec: &ExperimentSpec) -> Result<()> {
        let holdouts = sqlx::query_scalar!("SELECT id FROM eval_resource_revisions WHERE owner_id=$1 AND id=ANY($2) AND content->'content'->>'partition'='holdout'",
            owner.as_str(), &spec.cases.iter().map(|value| value.as_str().to_owned()).collect::<Vec<_>>()).fetch_all(&self.pool).await?;
        if holdouts.is_empty() {
            return Err(invalid(
                "Independent improvement claims require a holdout partition",
            ));
        }
        let consumed = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM eval_holdout_consumption WHERE owner_id=$1 AND case_revision_id=ANY($2))",
            owner.as_str(), &holdouts).fetch_one(&self.pool).await?.unwrap_or(false);
        if consumed {
            return Err(crate::experiments::conflict(
                "A fresh holdout revision is required for another independent-improvement claim",
            ));
        }
        Ok(())
    }

    pub(crate) async fn insert_executions(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: &EvalExperimentId,
        spec: &ExperimentSpec,
    ) -> Result<()> {
        for (variant, _) in spec.variants.iter().enumerate() {
            let variant = i32::try_from(variant)
                .map_err(|error| invalid(&format!("Too many variants: {error}")))?;
            for case in &spec.cases {
                for repetition in 0..spec.repetitions {
                    let execution_id = EvalExecutionId::generate();
                    let repetition = i32::try_from(repetition)
                        .map_err(|error| invalid(&format!("Repetition overflow: {error}")))?;
                    sqlx::query!("INSERT INTO eval_executions(id,experiment_id,variant_index,case_revision_id,repetition) VALUES($1,$2,$3,$4,$5)", execution_id.as_str(), id.as_str(), variant, case.as_str(), repetition)
            .execute(&mut **tx).await?;
                }
            }
        }
        Ok(())
    }

    pub async fn list(&self, owner: &UserId) -> Result<Vec<ExperimentRecord>> {
        Ok(sqlx::query_scalar!(
            r#"SELECT to_jsonb(e) || jsonb_build_object('accounting',to_jsonb(b)) AS "record!: Json<ExperimentRecord>" FROM eval_experiments e JOIN eval_budget_accounts b ON b.id=e.budget_id WHERE e.owner_id=$1 ORDER BY e.created_at DESC LIMIT 100"#,
            owner.as_str()
        ).fetch_all(&self.pool).await?.into_iter().map(|record| record.0).collect())
    }

    pub async fn get(&self, owner: &UserId, id: &EvalExperimentId) -> Result<ExperimentDetail> {
        let experiment = sqlx::query_scalar!(
            r#"SELECT to_jsonb(e) || jsonb_build_object('accounting',to_jsonb(b)) AS "record!: Json<ExperimentRecord>" FROM eval_experiments e JOIN eval_budget_accounts b ON b.id=e.budget_id WHERE e.id=$1 AND e.owner_id=$2"#,
            id.as_str(), owner.as_str()
        ).fetch_optional(&self.pool).await?.ok_or_else(|| crate::experiments::missing("Experiment unavailable in this scope"))?.0;
        let executions = sqlx::query_scalar!(
            r#"SELECT to_jsonb(x) AS "record!: Json<ExecutionRecord>" FROM eval_executions x WHERE experiment_id=$1 ORDER BY variant_index,case_revision_id,repetition"#,
            id.as_str()
        ).fetch_all(&self.pool).await?.into_iter().map(|record| record.0).collect();
        Ok(ExperimentDetail {
            experiment,
            executions,
        })
    }

    pub async fn cancel(&self, owner: &UserId, id: &EvalExperimentId) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        super::lock_owner(&mut tx, owner).await?;
        let changed = sqlx::query!("UPDATE eval_experiments SET status='cancelled' WHERE id=$1 AND owner_id=$2 AND status IN ('queued','running','cancelled','blocked') RETURNING id",
            id.as_str(), owner.as_str())
            .fetch_optional(&mut *tx).await?;
        changed.ok_or_else(|| crate::experiments::conflict("Experiment cannot be cancelled"))?;
        sqlx::query!("UPDATE eval_executions SET status='cancelled',finished_at=NOW(),lease_expires_at=NULL WHERE experiment_id=$1 AND status IN ('queued','running','awaiting_approval')", id.as_str())
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}
