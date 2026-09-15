//! Durable optimization policy and source-change provenance. Campaigns retain
//! immutable experiments; publication remains an independently reviewed action.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{EvalBudgetId, ManagedResourceId, ResourceRevisionId};

use crate::{EvaluationError, Result};

pub mod comparison;
pub mod diagnostics;
pub mod holdout;
mod record;
pub mod report;
pub mod repository;
pub mod suggestions;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CampaignPolicy {
    pub name: String,
    pub resource_id: ManagedResourceId,
    pub baseline_revision_id: ResourceRevisionId,
    pub budget_id: EvalBudgetId,
    pub objective: OptimizationObjective,
    pub minimum_quality_milli: u32,
    pub minimum_pairs: u32,
    pub maximum_iterations: u32,
    pub automatic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationObjective {
    Quality,
    Tokens,
    Cost,
    Latency,
}

impl CampaignPolicy {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty()
            || self.name.len() > 200
            || !(1000..=5000).contains(&self.minimum_quality_milli)
            || !(2..=1000).contains(&self.minimum_pairs)
            || !(1..=100).contains(&self.maximum_iterations)
        {
            return Err(EvaluationError::InvalidSpec("Campaign requires a name, quality floor, at least two pairs and a bounded iteration limit".to_owned()));
        }
        Ok(())
    }
}
