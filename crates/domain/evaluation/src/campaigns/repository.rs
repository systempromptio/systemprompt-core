//! Owner-scoped campaign persistence with optimistic concurrency and an audit
//! event for every state transition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::types::Json;
use systemprompt_identifiers::{EvalCampaignId, EvalExperimentId, UserId};

use super::CampaignPolicy;
use crate::Result;
use crate::experiments::{conflict, content_digest, missing};

#[derive(Debug, Clone)]
pub struct CampaignRepository {
    pub(super) pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CampaignRecord {
    pub id: EvalCampaignId,
    pub owner_id: UserId,
    pub created_by: UserId,
    pub policy: CampaignPolicy,
    pub status: String,
    pub generation: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CampaignAction {
    Pause,
    Resume,
    Complete,
    Cancel,
}

impl CampaignAction {
    const fn status(self) -> &'static str {
        match self {
            Self::Pause => "paused",
            Self::Resume => "active",
            Self::Complete => "completed",
            Self::Cancel => "cancelled",
        }
    }
}

impl CampaignRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        owner: &UserId,
        actor: &UserId,
        key: &str,
        policy: &CampaignPolicy,
    ) -> Result<EvalCampaignId> {
        let result = self.create_inner(owner, actor, key, policy).await;
        if let Err(error) = &result {
            self.record_diagnostic(
                owner,
                actor,
                None,
                &format!("setup:{}", content_digest(&key)?),
                super::diagnostics::DiagnosticStage::Setup,
                super::diagnostics::DiagnosticCode::from_error(error),
            )
            .await?;
        }
        result
    }

    async fn create_inner(
        &self,
        owner: &UserId,
        actor: &UserId,
        key: &str,
        policy: &CampaignPolicy,
    ) -> Result<EvalCampaignId> {
        policy.validate()?;
        if key.trim().is_empty() || key.len() > 200 {
            return Err(crate::experiments::invalid(
                "Campaign operation key is required",
            ));
        }
        let digest = content_digest(policy)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            format!("campaign:{}:{key}", owner.as_str())
        )
        .fetch_one(&mut *tx)
        .await?;
        if let Some(row) = sqlx::query!(
            "SELECT id,policy_digest FROM eval_campaigns WHERE owner_id=$1 AND operation_key=$2",
            owner.as_str(),
            key
        )
        .fetch_optional(&mut *tx)
        .await?
        {
            if row.policy_digest != digest {
                return Err(conflict(
                    "Campaign operation key was reused with different input",
                ));
            }
            return Ok(EvalCampaignId::new(row.id));
        }
        let budget = sqlx::query_scalar!(
            "SELECT id FROM eval_budget_accounts WHERE owner_id=$1 AND id=$2 AND NOT frozen",
            owner.as_str(),
            policy.budget_id.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?;
        if budget.is_none() {
            return Err(missing("Campaign budget unavailable"));
        }
        let id = EvalCampaignId::generate();
        sqlx::query!("INSERT INTO eval_campaigns(id,owner_id,created_by,resource_id,baseline_revision_id,budget_id,policy,policy_digest,operation_key) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)", id.as_str(), owner.as_str(), actor.as_str(), policy.resource_id.as_str(), policy.baseline_revision_id.as_str(), policy.budget_id.as_str(), Json(policy) as _, digest, key).execute(&mut *tx).await?;
        sqlx::query!("INSERT INTO eval_campaign_events(campaign_id,generation,actor_id,action) VALUES($1,0,$2,'created')", id.as_str(), actor.as_str()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(id)
    }

    pub async fn list(
        &self,
        owner: &UserId,
        after: Option<&EvalCampaignId>,
    ) -> Result<Vec<CampaignRecord>> {
        let rows = sqlx::query!("SELECT id,owner_id,created_by,policy,status,generation,created_at FROM eval_campaigns WHERE owner_id=$1 AND ($2::TEXT IS NULL OR id>$2) ORDER BY id LIMIT 51", owner.as_str(), after.map(EvalCampaignId::as_str)).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(CampaignRecord {
                    id: EvalCampaignId::new(row.id),
                    owner_id: UserId::new(row.owner_id),
                    created_by: UserId::new(row.created_by),
                    policy: serde_json::from_value(row.policy)?,
                    status: row.status,
                    generation: row.generation,
                    created_at: row.created_at,
                })
            })
            .collect()
    }

    pub async fn get(&self, owner: &UserId, id: &EvalCampaignId) -> Result<CampaignRecord> {
        let row = sqlx::query!("SELECT id,owner_id,created_by,policy,status,generation,created_at FROM eval_campaigns WHERE owner_id=$1 AND id=$2", owner.as_str(), id.as_str()).fetch_optional(&self.pool).await?.ok_or_else(|| missing("Campaign unavailable"))?;
        Ok(CampaignRecord {
            id: EvalCampaignId::new(row.id),
            owner_id: UserId::new(row.owner_id),
            created_by: UserId::new(row.created_by),
            policy: serde_json::from_value(row.policy)?,
            status: row.status,
            generation: row.generation,
            created_at: row.created_at,
        })
    }

    pub async fn transition(
        &self,
        owner: &UserId,
        actor: &UserId,
        id: &EvalCampaignId,
        command: (i64, CampaignAction),
    ) -> Result<()> {
        let (generation, action) = command;
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "SELECT id FROM eval_campaigns WHERE owner_id=$1 AND id=$2 FOR UPDATE",
            owner.as_str(),
            id.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| missing("Campaign unavailable"))?;
        if generation < 0 || generation == i64::MAX {
            return Err(conflict("Invalid campaign generation"));
        }
        let already=sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM eval_campaign_events e JOIN eval_campaigns c ON c.id=e.campaign_id WHERE c.owner_id=$1 AND c.id=$2 AND e.generation=$3 AND e.action=$4)",owner.as_str(),id.as_str(),generation+1,action.status()).fetch_one(&mut *tx).await?.unwrap_or(false);
        if already {
            return Ok(());
        }
        let changed = sqlx::query!("UPDATE eval_campaigns SET status=$4,generation=generation+1,updated_at=NOW() WHERE owner_id=$1 AND id=$2 AND generation=$3 AND status IN ('active','paused') AND status<>$4 RETURNING generation", owner.as_str(), id.as_str(), generation, action.status()).fetch_optional(&mut *tx).await?.ok_or_else(|| conflict("Campaign state or generation changed"))?;
        sqlx::query!("INSERT INTO eval_campaign_events(campaign_id,generation,actor_id,action) VALUES($1,$2,$3,$4)", id.as_str(), changed.generation, actor.as_str(), action.status()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn attach_experiment(
        &self,
        owner: &UserId,
        actor: &UserId,
        id: &EvalCampaignId,
        experiment: &EvalExperimentId,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let campaign = sqlx::query!("SELECT policy,budget_id,status,generation FROM eval_campaigns WHERE owner_id=$1 AND id=$2 FOR UPDATE", owner.as_str(), id.as_str()).fetch_optional(&mut *tx).await?.ok_or_else(|| missing("Campaign unavailable"))?;
        if sqlx::query_scalar!("SELECT iteration FROM eval_campaign_experiments WHERE campaign_id=$1 AND experiment_id=$2", id.as_str(), experiment.as_str()).fetch_optional(&mut *tx).await?.is_some() { return Ok(()); }
        let policy: CampaignPolicy = serde_json::from_value(campaign.policy)?;
        let count = sqlx::query_scalar!(
            "SELECT count(*) FROM eval_campaign_experiments WHERE campaign_id=$1",
            id.as_str()
        )
        .fetch_one(&mut *tx)
        .await?
        .unwrap_or(0);
        if campaign.status != "active" || count >= i64::from(policy.maximum_iterations) {
            return Err(conflict(
                "Campaign is inactive or its iteration limit is exhausted",
            ));
        }
        let budget = sqlx::query_scalar!(
            "SELECT budget_id FROM eval_experiments WHERE owner_id=$1 AND id=$2",
            owner.as_str(),
            experiment.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| missing("Experiment unavailable"))?;
        if budget != campaign.budget_id {
            return Err(conflict(
                "Every campaign experiment must use its shared budget",
            ));
        }
        let iteration =
            i32::try_from(count + 1).map_err(|_error| conflict("Iteration limit exceeded"))?;
        sqlx::query!("INSERT INTO eval_campaign_experiments(campaign_id,owner_id,experiment_id,iteration,created_by) VALUES($1,$2,$3,$4,$5)", id.as_str(), owner.as_str(), experiment.as_str(), iteration, actor.as_str()).execute(&mut *tx).await?;
        sqlx::query!(
            "UPDATE eval_campaigns SET generation=generation+1,updated_at=NOW() WHERE id=$1",
            id.as_str()
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!("INSERT INTO eval_campaign_events(campaign_id,generation,actor_id,action) VALUES($1,$2,$3,'experiment_attached')", id.as_str(), campaign.generation+1, actor.as_str()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_experiments(
        &self,
        owner: &UserId,
        id: &EvalCampaignId,
    ) -> Result<Vec<EvalExperimentId>> {
        self.get(owner, id).await?;
        Ok(sqlx::query_scalar!("SELECT experiment_id FROM eval_campaign_experiments WHERE owner_id=$1 AND campaign_id=$2 ORDER BY iteration", owner.as_str(), id.as_str()).fetch_all(&self.pool).await?.into_iter().map(EvalExperimentId::new).collect())
    }
}
