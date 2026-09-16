//! Campaign dispatch atomically reserves an iteration and creates its frozen
//! execution matrix, so a restart cannot leave an untracked paid experiment.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ExperimentRepository;
use crate::Result;
use crate::campaigns::CampaignPolicy;
use crate::experiments::{ExperimentSpec, conflict, content_digest, missing};
use sqlx::types::Json;
use systemprompt_identifiers::{EvalCampaignId, EvalExperimentId, UserId};

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CampaignExperiment {
    pub campaign_id: EvalCampaignId,
    pub idempotency_key: String,
    pub spec: ExperimentSpec,
}

impl ExperimentRepository {
    pub async fn create_for_campaign(
        &self,
        owner: &UserId,
        actor: &UserId,
        input: &CampaignExperiment,
    ) -> Result<EvalExperimentId> {
        self.admission.admit(&input.spec)?;
        if input.idempotency_key.trim().is_empty() || input.idempotency_key.len() > 100 {
            return Err(crate::experiments::invalid(
                "An operation key of at most 100 bytes is required",
            ));
        }
        let campaign = sqlx::query!(
            "SELECT policy FROM eval_campaigns WHERE owner_id=$1 AND id=$2",
            owner.as_str(),
            input.campaign_id.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| missing("Campaign unavailable"))?;
        let policy: CampaignPolicy = serde_json::from_value(campaign.policy)?;
        let digest = content_digest(&input.spec)?;
        let key = format!("campaign:{}:{}", input.campaign_id, input.idempotency_key);
        if let Some(existing) = sqlx::query!("SELECT e.id,e.spec_digest,c.campaign_id FROM eval_experiments e JOIN eval_campaign_experiments c ON c.experiment_id=e.id WHERE e.owner_id=$1 AND e.idempotency_key=$2", owner.as_str(), &key).fetch_optional(&self.pool).await? {
            if existing.spec_digest != digest || existing.campaign_id != input.campaign_id.as_str() { return Err(conflict("Campaign dispatch key conflicts with retained input")); }
            return Ok(EvalExperimentId::new(existing.id));
        }
        let preflight = self
            .preflight(owner, &policy.budget_id, &input.spec)
            .await?;
        if !preflight.affordable {
            return Err(crate::EvaluationError::budget_exhausted(&preflight));
        }
        self.insert_campaign_run(owner, actor, input).await
    }

    async fn insert_campaign_run(
        &self,
        owner: &UserId,
        actor: &UserId,
        input: &CampaignExperiment,
    ) -> Result<EvalExperimentId> {
        let digest = content_digest(&input.spec)?;
        let key = format!("campaign:{}:{}", input.campaign_id, input.idempotency_key);
        let mut tx = self.pool.begin().await?;
        super::lock_owner(&mut tx, owner).await?;
        let campaign = sqlx::query!(
            "SELECT policy,budget_id,status,generation FROM eval_campaigns WHERE owner_id=$1 AND id=$2 FOR UPDATE",
            owner.as_str(),
            input.campaign_id.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| missing("Campaign unavailable"))?;
        let policy: CampaignPolicy = serde_json::from_value(campaign.policy)?;
        if let Some(row) = sqlx::query!(
            "SELECT id,spec_digest FROM eval_experiments WHERE owner_id=$1 AND idempotency_key=$2",
            owner.as_str(),
            &key
        )
        .fetch_optional(&mut *tx)
        .await?
        {
            if row.spec_digest != digest {
                return Err(conflict(
                    "Campaign dispatch key conflicts with retained input",
                ));
            }
            return Ok(EvalExperimentId::new(row.id));
        }
        let count = sqlx::query_scalar!(
            "SELECT count(*) FROM eval_campaign_experiments WHERE campaign_id=$1",
            input.campaign_id.as_str()
        )
        .fetch_one(&mut *tx)
        .await?
        .unwrap_or(0);
        if campaign.status != "active" || count >= i64::from(policy.maximum_iterations) {
            return Err(conflict(
                "Campaign is inactive or has exhausted its iteration limit",
            ));
        }
        let id = EvalExperimentId::generate();
        sqlx::query!("INSERT INTO eval_experiments(id,owner_id,spec,spec_digest,budget_id,idempotency_key) VALUES($1,$2,$3,$4,$5,$6)", id.as_str(), owner.as_str(), Json(&input.spec) as _, digest, policy.budget_id.as_str(), key).execute(&mut *tx).await?;
        Self::insert_executions(&mut tx, &id, &input.spec).await?;
        super::holdout::consume(&mut tx, owner, &id, &input.spec).await?;
        if input.spec.claim_independent_improvement {
            sqlx::query!("INSERT INTO eval_holdout_consumption(owner_id,case_revision_id,experiment_id) SELECT $1,c.id,$2 FROM eval_resource_revisions c WHERE c.owner_id=$1 AND c.id=ANY($3) AND c.content->'content'->>'partition'='holdout'", owner.as_str(), id.as_str(), &input.spec.cases.iter().map(|case| case.as_str().to_owned()).collect::<Vec<_>>()).execute(&mut *tx).await?;
        }
        let iteration =
            i32::try_from(count + 1).map_err(|_error| conflict("Iteration limit exceeded"))?;
        sqlx::query!("INSERT INTO eval_campaign_experiments(campaign_id,owner_id,experiment_id,iteration,created_by) VALUES($1,$2,$3,$4,$5)", input.campaign_id.as_str(), owner.as_str(), id.as_str(), iteration, actor.as_str()).execute(&mut *tx).await?;
        sqlx::query!(
            "UPDATE eval_campaigns SET generation=generation+1,updated_at=NOW() WHERE id=$1",
            input.campaign_id.as_str()
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!("INSERT INTO eval_campaign_events(campaign_id,generation,actor_id,action) VALUES($1,$2,$3,'experiment_queued')", input.campaign_id.as_str(), campaign.generation + 1, actor.as_str()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(id)
    }
}
