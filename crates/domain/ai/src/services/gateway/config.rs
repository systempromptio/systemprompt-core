//! YAML schema for the declarative gateway-policy baseline.
//!
//! A deployment commits a [`GatewayPolicyConfig`] at
//! `services/gateway/policies.yaml` declaring the gateway policies every
//! instance should boot with. The bootstrap loader parses this struct, hands
//! it to [`super::ingestion::GatewayPolicyIngestionService`], and the service
//! projects it into `ai_gateway_policies`.
//!
//! The contract is one-way (YAML → DB), mirroring the access-control
//! ingestion path.

use serde::{Deserialize, Serialize};

use super::spec::GatewayPolicySpec;
use crate::error::RepositoryError;

const fn default_enabled() -> bool {
    true
}

/// Top-level shape of `services/gateway/policies.yaml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayPolicyConfig {
    #[serde(default)]
    pub policies: Vec<GatewayPolicyEntry>,
}

/// One declared gateway policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayPolicyEntry {
    /// Unique policy name — the upsert key for `ai_gateway_policies`.
    pub name: String,
    /// Whether the policy is active. Disabled policies are still upserted so
    /// they can be re-enabled without losing their `spec`.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// The policy body — allow-list, token ceilings, quotas, safety config.
    #[serde(default)]
    pub spec: GatewayPolicySpec,
}

impl GatewayPolicyConfig {
    /// Reject empty or duplicate policy names before ingestion.
    pub fn validate(&self) -> Result<(), RepositoryError> {
        let mut seen = std::collections::HashSet::with_capacity(self.policies.len());
        for (idx, policy) in self.policies.iter().enumerate() {
            if policy.name.trim().is_empty() {
                return Err(RepositoryError::InvalidData {
                    field: format!("policies[{idx}].name"),
                    reason: "policy name must not be empty".to_owned(),
                });
            }
            if !seen.insert(policy.name.as_str()) {
                return Err(RepositoryError::InvalidData {
                    field: format!("policies[{idx}].name"),
                    reason: format!("duplicate policy name '{}'", policy.name),
                });
            }
        }
        Ok(())
    }
}
