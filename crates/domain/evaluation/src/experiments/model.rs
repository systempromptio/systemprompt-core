//! Frozen, validated client comparison specifications.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{EvalRevisionId, ModelId, ProviderId};

use super::invalid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ClientKind {
    ClaudeCode,
    #[serde(alias = "open-code")]
    Opencode,
    Codex,
    Hermes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Fixture,
    Live,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Objective {
    Quality,
    Tokens,
    Cost,
    Latency,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExperimentSpec {
    pub schema_version: u32,
    pub name: String,
    pub cases: Vec<EvalRevisionId>,
    pub rubric: EvalRevisionId,
    #[serde(default)]
    pub dataset: Option<EvalRevisionId>,
    pub variants: Vec<VariantSpec>,
    pub repetitions: u32,
    pub budget_microdollars: i64,
    pub execution_mode: ExecutionMode,
    pub objective: Objective,
    #[serde(default)]
    pub frozen: Option<FrozenSettings>,
    #[serde(default)]
    pub claim_independent_improvement: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FrozenSettings {
    pub provider_prices_digest: String,
    pub tool_configuration_digest: String,
    pub fixture_clock: String,
    pub fixture_timezone: String,
    pub permissions_digest: String,
    pub dataset_digest: String,
    pub rubric_digest: String,
    pub cost_envelope: FrozenCostEnvelope,
}

/// The price digest proves which configured provider/tool price snapshot
/// supplied the frozen maximum-cost inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FrozenCostEnvelope {
    pub maximum_attempts_per_execution: u32,
    pub generation_microdollars_per_attempt: i64,
    pub judging_microdollars_per_attempt: i64,
    pub tool_microdollars_per_attempt: i64,
    pub suggestion_calls: u32,
    pub suggestion_microdollars_per_call: i64,
    pub auxiliary_calls: u32,
    pub auxiliary_microdollars_per_call: i64,
}

impl FrozenCostEnvelope {
    pub fn validate(&self) -> Result<()> {
        if !(1..=3).contains(&self.maximum_attempts_per_execution)
            || [
                self.generation_microdollars_per_attempt,
                self.judging_microdollars_per_attempt,
                self.tool_microdollars_per_attempt,
                self.suggestion_microdollars_per_call,
                self.auxiliary_microdollars_per_call,
            ]
            .iter()
            .any(|value| *value < 0)
            || self.suggestion_calls > 100
            || self.auxiliary_calls > 100
        {
            return Err(invalid(
                "Frozen cost envelope contains invalid prices or call counts",
            ));
        }
        Ok(())
    }

    pub fn maximum_microdollars(&self, executions: u64) -> Result<i64> {
        self.validate()?;
        let per_attempt = self
            .generation_microdollars_per_attempt
            .checked_add(self.judging_microdollars_per_attempt)
            .and_then(|value| value.checked_add(self.tool_microdollars_per_attempt))
            .ok_or_else(|| invalid("Frozen cost envelope overflow"))?;
        let execution_total = i64::try_from(executions)
            .ok()
            .and_then(|count| count.checked_mul(i64::from(self.maximum_attempts_per_execution)))
            .and_then(|count| count.checked_mul(per_attempt))
            .ok_or_else(|| invalid("Frozen execution cost overflow"))?;
        let suggestions = i64::from(self.suggestion_calls)
            .checked_mul(self.suggestion_microdollars_per_call)
            .ok_or_else(|| invalid("Frozen suggestion cost overflow"))?;
        let auxiliary = i64::from(self.auxiliary_calls)
            .checked_mul(self.auxiliary_microdollars_per_call)
            .ok_or_else(|| invalid("Frozen auxiliary cost overflow"))?;
        execution_total
            .checked_add(suggestions)
            .and_then(|value| value.checked_add(auxiliary))
            .ok_or_else(|| invalid("Frozen total cost overflow"))
    }
}

impl FrozenSettings {
    pub fn validate(&self) -> Result<()> {
        self.cost_envelope.validate()?;
        if [
            &self.provider_prices_digest,
            &self.tool_configuration_digest,
            &self.permissions_digest,
            &self.dataset_digest,
            &self.rubric_digest,
        ]
        .iter()
        .any(|digest| {
            digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        }) || self
            .fixture_clock
            .parse::<chrono::DateTime<chrono::FixedOffset>>()
            .is_err()
            || self.fixture_timezone.trim().is_empty()
            || self.fixture_timezone.len() > 64
        {
            return Err(invalid(
                "Frozen settings require exact digests, an RFC3339 clock and timezone",
            ));
        }
        Ok(())
    }
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
        if let Some(frozen) = &self.frozen {
            frozen.validate()?;
        }
        Ok(())
    }
}
