//! Retained blocked work is retrievable even when the triggering request
//! failed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::repository::CampaignRepository;
use crate::{EvaluationError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{EvalCampaignId, UserId};

/// Campaign step that requires an operator action before it can proceed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStage {
    Setup,
    Launch,
    AutomaticFollowup,
    Report,
    Holdout,
}

/// Safe actionable failure categories; source credentials and raw errors are
/// not retained.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    UnsupportedCapability,
    MissingTemplate,
    MissingSuggestion,
    IncompleteEvidence,
    InvalidInput,
    BudgetUnavailable,
    StorageUnavailable,
    IterationLimit,
}

impl DiagnosticCode {
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::UnsupportedCapability => {
                "Choose a client, platform and exact version with verified native isolation and metering; refresh evaluator capabilities."
            },
            Self::MissingTemplate => {
                "Select a retained paired experiment with a frozen dataset, rubric and matched baseline/candidate environment."
            },
            Self::MissingSuggestion => {
                "Review the development report and retain an actionable development-only candidate suggestion before retrying."
            },
            Self::IncompleteEvidence => {
                "Complete paired development measurements and settle accounting; use fresh independent holdout cases for confirmation."
            },
            Self::InvalidInput => {
                "Review the retained baseline, candidate ownership, dataset partitions and idempotency key, then retry with corrected input."
            },
            Self::BudgetUnavailable => {
                "Review the frozen cost envelope and remaining shared budget before authorizing another run."
            },
            Self::StorageUnavailable => {
                "Restore database availability and retry the same operation key; inspect processing health."
            },
            Self::IterationLimit => {
                "The authorized iteration limit is exhausted; review the campaign policy before planning additional work."
            },
        }
    }
    pub fn from_error(error: &EvaluationError) -> Self {
        match error {
            EvaluationError::BudgetExhausted { .. } => Self::BudgetUnavailable,
            EvaluationError::ResourceNotFound(_) => Self::MissingTemplate,
            EvaluationError::Repository(_)
            | EvaluationError::Trace(_)
            | EvaluationError::ManagedRevisions(_) => Self::StorageUnavailable,
            EvaluationError::InvalidSpec(message) if message.contains("unsupported") => {
                Self::UnsupportedCapability
            },
            _ => Self::InvalidInput,
        }
    }
}

/// Durable diagnostic with repeat count and resolution history.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CampaignDiagnostic {
    pub id: String,
    pub campaign_id: Option<EvalCampaignId>,
    pub operation_key: String,
    pub stage: DiagnosticStage,
    pub code: DiagnosticCode,
    pub remediation: String,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub occurrences: i64,
    pub resolved_at: Option<DateTime<Utc>>,
}

/// One retained operator-facing diagnostic: who hit it, where and what.
#[derive(Debug, Clone, Copy)]
pub struct DiagnosticRecord<'a> {
    pub actor: &'a UserId,
    pub campaign: Option<&'a EvalCampaignId>,
    pub operation: &'a str,
    pub stage: DiagnosticStage,
    pub code: DiagnosticCode,
}

impl CampaignRepository {
    pub async fn record_diagnostic(
        &self,
        owner: &UserId,
        record: DiagnosticRecord<'_>,
    ) -> Result<String> {
        let DiagnosticRecord {
            actor,
            campaign,
            operation,
            stage,
            code,
        } = record;
        if operation.is_empty() || operation.len() > 200 {
            return Err(crate::experiments::invalid(
                "Diagnostic operation key must contain 1–200 bytes",
            ));
        }
        if let Some(campaign) = campaign {
            self.get(owner, campaign).await?;
        }
        let id = EvalCampaignId::generate().to_string();
        let stage = serde_json::to_string(&stage)?;
        let code_value = serde_json::to_string(&code)?;
        let stage = stage.trim_matches('"');
        let code_key = code_value.trim_matches('"');
        Ok(sqlx::query_scalar!("INSERT INTO eval_campaign_diagnostics(id,owner_id,campaign_id,operation_key,stage,code,remediation,actor_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(owner_id,operation_key,stage,code) DO UPDATE SET occurrences=eval_campaign_diagnostics.occurrences+1,last_seen_at=clock_timestamp(),resolved_at=NULL RETURNING id",id,owner.as_str(),campaign.map(EvalCampaignId::as_str),operation,stage,code_key,code.remediation(),actor.as_str()).fetch_one(&self.pool).await?)
    }
    pub async fn diagnostics(
        &self,
        owner: &UserId,
        campaign: Option<&EvalCampaignId>,
        after: Option<&str>,
        limit: u32,
    ) -> Result<Vec<CampaignDiagnostic>> {
        if !(1..=100).contains(&limit) {
            return Err(crate::experiments::invalid(
                "Diagnostic page limit must be 1–100",
            ));
        }
        if let Some(id) = campaign {
            self.get(owner, id).await?;
        }
        let rows=sqlx::query!("SELECT id,campaign_id,operation_key,stage,code,remediation,first_seen_at,last_seen_at,occurrences,resolved_at FROM eval_campaign_diagnostics WHERE owner_id=$1 AND ($2::text IS NULL OR campaign_id=$2) AND ($3::text IS NULL OR id>$3) ORDER BY id LIMIT $4",owner.as_str(),campaign.map(EvalCampaignId::as_str),after,i64::from(limit)).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(CampaignDiagnostic {
                    id: row.id,
                    campaign_id: row.campaign_id.map(EvalCampaignId::new),
                    operation_key: row.operation_key,
                    stage: serde_json::from_str(&serde_json::to_string(&row.stage)?)?,
                    code: serde_json::from_str(&serde_json::to_string(&row.code)?)?,
                    remediation: row.remediation,
                    first_seen_at: row.first_seen_at,
                    last_seen_at: row.last_seen_at,
                    occurrences: row.occurrences,
                    resolved_at: row.resolved_at,
                })
            })
            .collect()
    }
    pub async fn resolve_diagnostics(
        &self,
        owner: &UserId,
        campaign: &EvalCampaignId,
        operation: &str,
    ) -> Result<()> {
        sqlx::query!("UPDATE eval_campaign_diagnostics SET resolved_at=clock_timestamp() WHERE owner_id=$1 AND campaign_id=$2 AND operation_key=$3 AND resolved_at IS NULL",owner.as_str(),campaign.as_str(),operation).execute(&self.pool).await?;
        Ok(())
    }
}
