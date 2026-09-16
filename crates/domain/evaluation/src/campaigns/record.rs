//! Campaign rows as the repository decodes them: the typed record, the
//! reviewer's transition command and the status each action lands on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use systemprompt_identifiers::{EvalCampaignId, UserId};

use super::CampaignPolicy;
use crate::models::CampaignStatus;
use crate::{EvaluationError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CampaignRecord {
    pub id: EvalCampaignId,
    pub owner_id: UserId,
    pub created_by: UserId,
    pub policy: CampaignPolicy,
    pub status: CampaignStatus,
    pub generation: i64,
    pub publication_generation: Option<i64>,
    pub composed_hash: Option<String>,
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
    #[must_use]
    pub const fn status(self) -> CampaignStatus {
        match self {
            Self::Pause => CampaignStatus::Paused,
            Self::Resume => CampaignStatus::Active,
            Self::Complete => CampaignStatus::Completed,
            Self::Cancel => CampaignStatus::Cancelled,
        }
    }
}

/// A state transition guarded by the generation the caller last observed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CampaignTransition {
    pub expected_generation: i64,
    pub action: CampaignAction,
}

pub(super) struct CampaignRow {
    pub(super) id: String,
    pub(super) owner_id: String,
    pub(super) created_by: String,
    pub(super) policy: Json<CampaignPolicy>,
    pub(super) status: String,
    pub(super) generation: i64,
    pub(super) publication_generation: Option<i64>,
    pub(super) composed_hash: Option<String>,
    pub(super) created_at: DateTime<Utc>,
}

impl TryFrom<CampaignRow> for CampaignRecord {
    type Error = EvaluationError;

    fn try_from(row: CampaignRow) -> Result<Self> {
        Ok(Self {
            id: EvalCampaignId::new(row.id),
            owner_id: UserId::new(row.owner_id),
            created_by: UserId::new(row.created_by),
            policy: row.policy.0,
            status: CampaignStatus::parse(&row.status)?,
            generation: row.generation,
            publication_generation: row.publication_generation,
            composed_hash: row.composed_hash,
            created_at: row.created_at,
        })
    }
}
