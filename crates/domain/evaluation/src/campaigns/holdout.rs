//! Immutable proposals separate independent holdout review from execution
//! admission.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::repository::CampaignRepository;
use crate::Result;
use crate::experiments::{ExperimentSpec, conflict, content_digest, invalid, missing};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{EvalCampaignId, EvalExperimentId, UserId};

/// Frozen paired confirmation matrix retained before human authorization.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HoldoutProposal {
    pub id: String,
    pub campaign_id: EvalCampaignId,
    pub development_experiment_id: EvalExperimentId,
    pub operation_key: String,
    pub spec: ExperimentSpec,
    pub spec_digest: String,
    pub development_cases: i32,
    pub holdout_cases: i32,
    pub created_at: DateTime<Utc>,
    pub confirmed_by: Option<UserId>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub experiment_id: Option<EvalExperimentId>,
}
impl CampaignRepository {
    pub async fn propose_holdout(
        &self,
        owner: &UserId,
        campaign: &EvalCampaignId,
        development: &EvalExperimentId,
        key: &str,
        spec: &ExperimentSpec,
        counts: (i32, i32),
    ) -> Result<HoldoutProposal> {
        self.get(owner, campaign).await?;
        if key.trim().is_empty() || key.len() > 100 || counts.0 < 2 || counts.1 < 2 {
            return Err(invalid(
                "Holdout proposals require bounded operation keys and independent paired cases",
            ));
        }
        if !self
            .list_experiments(owner, campaign)
            .await?
            .contains(development)
        {
            return Err(missing("Development experiment is outside this campaign"));
        }
        let digest = content_digest(spec)?;
        let id = EvalCampaignId::generate().to_string();
        let stored=sqlx::query!("INSERT INTO eval_campaign_holdout_proposals(id,owner_id,campaign_id,development_experiment_id,operation_key,spec,spec_digest,development_cases,holdout_cases) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(owner_id,campaign_id,operation_key) DO UPDATE SET operation_key=EXCLUDED.operation_key RETURNING id,spec_digest,development_experiment_id",id,owner.as_str(),campaign.as_str(),development.as_str(),key,sqlx::types::Json(spec) as _,digest,counts.0,counts.1).fetch_one(&self.pool).await?;
        if stored.spec_digest != digest || stored.development_experiment_id != development.as_str()
        {
            return Err(conflict(
                "Holdout operation key conflicts with its retained proposal",
            ));
        }
        self.holdout_proposal(owner, campaign, &stored.id).await
    }
    pub async fn holdout_proposal(
        &self,
        owner: &UserId,
        campaign: &EvalCampaignId,
        id: &str,
    ) -> Result<HoldoutProposal> {
        Ok(sqlx::query_scalar!(r#"SELECT to_jsonb(p) AS "proposal!: sqlx::types::Json<HoldoutProposal>" FROM eval_campaign_holdout_proposals p WHERE owner_id=$1 AND campaign_id=$2 AND id=$3"#,owner.as_str(),campaign.as_str(),id).fetch_optional(&self.pool).await?.ok_or_else(||missing("Holdout proposal unavailable"))?.0)
    }
    pub async fn confirm_holdout(
        &self,
        owner: &UserId,
        actor: &UserId,
        campaign: &EvalCampaignId,
        id: &str,
        digest: &str,
    ) -> Result<HoldoutProposal> {
        let updated=sqlx::query!("UPDATE eval_campaign_holdout_proposals SET confirmed_by=COALESCE(confirmed_by,$4),confirmed_at=COALESCE(confirmed_at,clock_timestamp()) WHERE owner_id=$1 AND campaign_id=$2 AND id=$3 AND spec_digest=$5 RETURNING id",owner.as_str(),campaign.as_str(),id,actor.as_str(),digest).fetch_optional(&self.pool).await?;
        if updated.is_none() {
            return Err(conflict(
                "Confirmation must match the retained proposal digest",
            ));
        }
        self.holdout_proposal(owner, campaign, id).await
    }
    pub async fn attach_holdout_run(
        &self,
        owner: &UserId,
        campaign: &EvalCampaignId,
        id: &str,
        experiment: &EvalExperimentId,
    ) -> Result<HoldoutProposal> {
        sqlx::query!("UPDATE eval_campaign_holdout_proposals SET experiment_id=$4 WHERE owner_id=$1 AND campaign_id=$2 AND id=$3 AND confirmed_at IS NOT NULL AND (experiment_id IS NULL OR experiment_id=$4)",owner.as_str(),campaign.as_str(),id,experiment.as_str()).execute(&self.pool).await?;
        self.holdout_proposal(owner, campaign, id).await
    }
}
