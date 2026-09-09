//! Frozen, validated client comparison specifications.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{EvalRevisionId, ModelId, ProviderId};

use super::invalid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientKind {
    ClaudeCode,
    Opencode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Fixture,
    Live,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Objective {
    Quality,
    Cost,
    Latency,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariantSpec {
    pub client: ClientKind,
    pub client_version: String,
    pub model: ModelId,
    pub provider: ProviderId,
    pub skill_bundle_digest: String,
    pub configuration_digest: String,
    pub worker_image_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExperimentSpec {
    pub schema_version: u32,
    pub name: String,
    pub cases: Vec<EvalRevisionId>,
    pub rubric: EvalRevisionId,
    pub variants: Vec<VariantSpec>,
    pub repetitions: u32,
    pub budget_microdollars: i64,
    pub execution_mode: ExecutionMode,
    pub objective: Objective,
}

impl ExperimentSpec {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1 || self.name.trim().is_empty() || self.name.len() > 200 {
            return Err(invalid(
                "Expected schema version 1 and a name of 1–200 bytes",
            ));
        }
        if self.cases.is_empty()
            || self.cases.len() > 100
            || self.variants.is_empty()
            || self.variants.len() > 16
            || !(1..=10).contains(&self.repetitions)
        {
            return Err(invalid(
                "Expected 1–100 cases, 1–16 variants and 1–10 repetitions",
            ));
        }
        if self.budget_microdollars <= 0 {
            return Err(invalid(
                "Specify a positive budget, fixture/live mode and quality/cost/latency objective",
            ));
        }
        for (index, case) in self.cases.iter().enumerate() {
            if self.cases[..index].contains(case) {
                return Err(invalid("Duplicate case"));
            }
        }
        for (index, variant) in self.variants.iter().enumerate() {
            if self.variants[..index].contains(variant) {
                return Err(invalid(
                    "Duplicate variant; use repetitions for repeated executions",
                ));
            }
            if [
                variant.client_version.as_str(),
                variant.model.as_str(),
                variant.provider.as_str(),
            ]
            .iter()
            .any(|s| s.trim().is_empty())
                || [
                    &variant.skill_bundle_digest,
                    &variant.configuration_digest,
                    &variant.worker_image_digest,
                ]
                .iter()
                .any(|s| s.len() != 64 || !s.bytes().all(|c| c.is_ascii_hexdigit()))
            {
                return Err(invalid(
                    "Pin a supported client, model, provider and SHA-256 bundle/configuration/image digests",
                ));
            }
        }
        Ok(())
    }
}
